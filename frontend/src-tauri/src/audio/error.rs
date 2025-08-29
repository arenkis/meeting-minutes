use std::time::Duration;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use thiserror::Error;
use log::{error, warn, info, debug};

/// Comprehensive error types for audio system
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum AudioError {
    #[error("Device error: {message}")]
    Device { message: String, recoverable: bool },
    
    #[error("Channel error: {message}")]
    Channel { message: String, error_type: ChannelErrorType },
    
    #[error("Buffer error: {message}")]
    Buffer { message: String, buffer_type: String },
    
    #[error("VAD processing error: {message}")]
    VadProcessing { message: String, samples_lost: usize },
    
    #[error("Transcription error: {message}")]
    Transcription { message: String, chunk_id: u64 },
    
    #[error("Recovery error: {message}")]
    Recovery { message: String, attempts: u32 },
    
    #[error("Configuration error: {message}")]
    Configuration { message: String, field: String },
    
    #[error("Resource exhaustion: {message}")]
    ResourceExhaustion { message: String, resource_type: String },
    
    #[error("Timeout error: {message}")]
    Timeout { message: String, duration_ms: u64 },
    
    #[error("System error: {message}")]
    System { message: String, code: Option<i32> },
    
    #[error("Processing error: {message}")]
    Processing { message: String, context: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelErrorType {
    Closed,
    Full,
    SendFailed,
    ReceiveFailed,
    Recovery,
}

/// Error recovery strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorRecoveryStrategy {
    /// Retry immediately with exponential backoff
    Retry { max_attempts: u32, base_delay_ms: u64 },
    /// Fail gracefully and continue with reduced functionality
    Graceful { fallback_enabled: bool },
    /// Stop the affected component
    Stop,
    /// Restart the affected component
    Restart,
    /// Escalate to user intervention
    Escalate,
}

/// Actions that can be taken in response to an error
#[derive(Debug, Clone, Serialize)]
pub enum ErrorRecoveryAction {
    /// Retry with exponential backoff
    Retry { delay_ms: u64, attempt: u32 },
    /// Wait and retry with backoff
    Backoff { delay_ms: u64, attempt: u32 },
    /// Reset the component
    Reset,
    /// Ignore this error
    Ignore,
    /// Stop the component
    Stop,
    /// Restart the component
    Restart,
    /// Escalate to user intervention
    Escalate,
    /// Continue processing despite error
    Continue {
        with_degradation: bool,
        fallback_enabled: bool,
    },
}

/// Error context for better debugging
#[derive(Debug, Clone, Serialize)]
pub struct ErrorContext {
    pub component: String,
    pub operation: String,
    pub timestamp: u64,
    pub device_info: Option<DeviceErrorInfo>,
    pub system_info: SystemErrorInfo,
    pub recovery_info: Option<RecoveryInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceErrorInfo {
    pub device_name: String,
    pub device_type: String,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemErrorInfo {
    pub memory_usage_mb: u64,
    pub cpu_usage_percent: f32,
    pub active_streams: u32,
    pub buffer_utilization: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecoveryInfo {
    pub strategy: ErrorRecoveryStrategy,
    pub attempt_count: u32,
    pub last_attempt_ms: u64,
    pub success_rate: f32,
}

/// Error handler for managing and recovering from errors
pub struct ErrorHandler {
    error_counts: Arc<RwLock<std::collections::HashMap<String, AtomicU32>>>,
    recovery_strategies: Arc<RwLock<std::collections::HashMap<String, ErrorRecoveryStrategy>>>,
    error_callbacks: Arc<RwLock<Vec<Box<dyn Fn(&AudioError, &ErrorContext) + Send + Sync>>>>,
    max_error_history: usize,
    error_history: Arc<RwLock<std::collections::VecDeque<(AudioError, ErrorContext)>>>,
}

impl ErrorHandler {
    pub fn new() -> Self {
        let mut strategies = std::collections::HashMap::new();
        
        // Default recovery strategies
        strategies.insert(
            "device".to_string(),
            ErrorRecoveryStrategy::Retry { max_attempts: 3, base_delay_ms: 1000 }
        );
        strategies.insert(
            "channel".to_string(),
            ErrorRecoveryStrategy::Retry { max_attempts: 5, base_delay_ms: 500 }
        );
        strategies.insert(
            "buffer".to_string(),
            ErrorRecoveryStrategy::Graceful { fallback_enabled: true }
        );
        strategies.insert(
            "vad".to_string(),
            ErrorRecoveryStrategy::Graceful { fallback_enabled: true }
        );
        strategies.insert(
            "transcription".to_string(),
            ErrorRecoveryStrategy::Retry { max_attempts: 2, base_delay_ms: 2000 }
        );
        
        Self {
            error_counts: Arc::new(RwLock::new(std::collections::HashMap::new())),
            recovery_strategies: Arc::new(RwLock::new(strategies)),
            error_callbacks: Arc::new(RwLock::new(Vec::new())),
            max_error_history: 1000,
            error_history: Arc::new(RwLock::new(std::collections::VecDeque::new())),
        }
    }
    
    /// Handle an error with automatic recovery
    pub async fn handle_error(&self, error: AudioError, context: ErrorContext) -> ErrorRecoveryAction {
        // Log the error
        self.log_error(&error, &context).await;
        
        // Store in history
        self.store_error_history(error.clone(), context.clone()).await;
        
        // Increment error count
        self.increment_error_count(&context.component).await;
        
        // Determine recovery strategy
        let strategy = self.get_recovery_strategy(&context.component).await;
        
        // Execute recovery
        let action = self.execute_recovery(&error, &context, &strategy).await;
        
        // Notify callbacks
        self.notify_callbacks(&error, &context).await;
        
        action
    }
    
    /// Log error with appropriate level
    async fn log_error(&self, error: &AudioError, context: &ErrorContext) {
        let error_count = self.get_error_count(&context.component).await;
        
        match error {
            AudioError::Device { recoverable: true, .. } => {
                warn!("[{}] Device error (count: {}): {}", context.component, error_count, error);
            }
            AudioError::Device { recoverable: false, .. } => {
                error!("[{}] Critical device error (count: {}): {}", context.component, error_count, error);
            }
            AudioError::Channel { .. } => {
                warn!("[{}] Channel error (count: {}): {}", context.component, error_count, error);
            }
            AudioError::Buffer { .. } => {
                if error_count > 5 {
                    error!("[{}] Repeated buffer error (count: {}): {}", context.component, error_count, error);
                } else {
                    warn!("[{}] Buffer error (count: {}): {}", context.component, error_count, error);
                }
            }
            AudioError::ResourceExhaustion { .. } => {
                error!("[{}] Resource exhaustion (count: {}): {}", context.component, error_count, error);
            }
            _ => {
                info!("[{}] Error (count: {}): {}", context.component, error_count, error);
            }
        }
        
        // Log context details in debug mode
        debug!("[{}] Error context: {:?}", context.component, context);
    }
    
    /// Store error in history
    async fn store_error_history(&self, error: AudioError, context: ErrorContext) {
        let mut history = self.error_history.write().await;
        
        // Add new error
        history.push_back((error, context));
        
        // Maintain size limit
        while history.len() > self.max_error_history {
            history.pop_front();
        }
    }
    
    /// Increment error count for component
    async fn increment_error_count(&self, component: &str) {
        let mut counts = self.error_counts.write().await;
        let counter = counts.entry(component.to_string())
            .or_insert_with(|| AtomicU32::new(0));
        counter.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Get error count for component
    async fn get_error_count(&self, component: &str) -> u32 {
        let counts = self.error_counts.read().await;
        counts.get(component)
            .map(|counter| counter.load(Ordering::Relaxed))
            .unwrap_or(0)
    }
    
    /// Get recovery strategy for component
    async fn get_recovery_strategy(&self, component: &str) -> ErrorRecoveryStrategy {
        let strategies = self.recovery_strategies.read().await;
        strategies.get(component)
            .cloned()
            .unwrap_or(ErrorRecoveryStrategy::Graceful { fallback_enabled: false })
    }
    
    /// Execute recovery based on strategy
    async fn execute_recovery(
        &self,
        error: &AudioError,
        context: &ErrorContext,
        strategy: &ErrorRecoveryStrategy,
    ) -> ErrorRecoveryAction {
        match strategy {
            ErrorRecoveryStrategy::Retry { max_attempts, base_delay_ms } => {
                let error_count = self.get_error_count(&context.component).await;
                
                if error_count <= *max_attempts {
                    let delay = Duration::from_millis(*base_delay_ms * 2_u64.pow(error_count.min(10)));
                    info!("[{}] Scheduling retry in {:?} (attempt {}/{})", 
                          context.component, delay, error_count, max_attempts);
                    ErrorRecoveryAction::Retry { 
                        delay_ms: delay.as_millis() as u64,
                        attempt: error_count
                    }
                } else {
                    warn!("[{}] Max retry attempts exceeded, escalating", context.component);
                    ErrorRecoveryAction::Escalate
                }
            }
            ErrorRecoveryStrategy::Graceful { fallback_enabled } => {
                info!("[{}] Graceful degradation (fallback: {})", context.component, fallback_enabled);
                ErrorRecoveryAction::Continue { 
                    with_degradation: true,
                    fallback_enabled: *fallback_enabled,
                }
            }
            ErrorRecoveryStrategy::Stop => {
                warn!("[{}] Stopping component due to error", context.component);
                ErrorRecoveryAction::Stop
            }
            ErrorRecoveryStrategy::Restart => {
                info!("[{}] Restarting component", context.component);
                ErrorRecoveryAction::Restart
            }
            ErrorRecoveryStrategy::Escalate => {
                error!("[{}] Escalating to user intervention", context.component);
                ErrorRecoveryAction::Escalate
            }
        }
    }
    
    /// Notify error callbacks
    async fn notify_callbacks(&self, error: &AudioError, context: &ErrorContext) {
        let callbacks = self.error_callbacks.read().await;
        
        for callback in callbacks.iter() {
            callback(error, context);
        }
    }
    
    /// Add error callback
    pub async fn add_callback<F>(&self, callback: F) 
    where
        F: Fn(&AudioError, &ErrorContext) + Send + Sync + 'static,
    {
        let mut callbacks = self.error_callbacks.write().await;
        callbacks.push(Box::new(callback));
    }
    
    /// Set recovery strategy for component
    pub async fn set_recovery_strategy(&self, component: String, strategy: ErrorRecoveryStrategy) {
        let mut strategies = self.recovery_strategies.write().await;
        strategies.insert(component, strategy);
    }
    
    /// Reset error count for component
    pub async fn reset_error_count(&self, component: &str) {
        let mut counts = self.error_counts.write().await;
        if let Some(counter) = counts.get(component) {
            counter.store(0, Ordering::Relaxed);
            info!("[{}] Error count reset", component);
        }
    }
    
    /// Get error statistics
    pub async fn get_error_statistics(&self) -> ErrorStatistics {
        let counts = self.error_counts.read().await;
        let history = self.error_history.read().await;
        
        let mut component_errors = std::collections::HashMap::new();
        for (component, counter) in counts.iter() {
            component_errors.insert(component.clone(), counter.load(Ordering::Relaxed));
        }
        
        let total_errors = component_errors.values().sum();
        let recent_errors = history.iter()
            .rev()
            .take(100) // Last 100 errors
            .count() as u32;
        
        ErrorStatistics {
            total_errors,
            recent_errors,
            component_errors,
            error_history_size: history.len(),
        }
    }
    
    /// Check if component should be considered failed
    pub async fn is_component_failed(&self, component: &str, failure_threshold: u32) -> bool {
        self.get_error_count(component).await >= failure_threshold
    }
    
    /// Get recent errors for analysis
    pub async fn get_recent_errors(&self, limit: usize) -> Vec<(AudioError, ErrorContext)> {
        let history = self.error_history.read().await;
        history.iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }
}


/// Error statistics for monitoring
#[derive(Debug, Clone, Serialize)]
pub struct ErrorStatistics {
    pub total_errors: u32,
    pub recent_errors: u32,
    pub component_errors: std::collections::HashMap<String, u32>,
    pub error_history_size: usize,
}

/// Helper functions for creating common errors
impl AudioError {
    pub fn device_disconnected(device_name: &str) -> Self {
        AudioError::Device {
            message: format!("Device '{}' disconnected", device_name),
            recoverable: true,
        }
    }
    
    pub fn channel_closed(channel_id: &str) -> Self {
        AudioError::Channel {
            message: format!("Channel '{}' closed unexpectedly", channel_id),
            error_type: ChannelErrorType::Closed,
        }
    }
    
    pub fn buffer_overflow(buffer_size: usize, attempted_size: usize) -> Self {
        AudioError::Buffer {
            message: format!("Buffer overflow: size {} exceeded by {}", buffer_size, attempted_size),
            buffer_type: "audio".to_string(),
        }
    }
    
    pub fn vad_processing_failed(samples_lost: usize, reason: &str) -> Self {
        AudioError::VadProcessing {
            message: format!("VAD processing failed: {}", reason),
            samples_lost,
        }
    }
    
    pub fn transcription_failed(samples: usize, reason: &str) -> Self {
        AudioError::Processing {
            message: format!("Transcription failed for {} samples: {}", samples, reason),
            context: Some(format!("samples_lost: {}, recoverable: true", samples)),
        }
    }

    pub fn chunk_processing_failed(samples: usize, reason: &str) -> Self {
        AudioError::Processing {
            message: format!("Chunk processing failed for {} samples: {}", samples, reason),
            context: Some(format!("samples_lost: {}, recoverable: true", samples)),
        }
    }

    pub fn processing_timeout(samples: usize, timeout_ms: u64) -> Self {
        AudioError::Processing {
            message: format!("Processing timeout after {}ms for {} samples", timeout_ms, samples),
            context: Some(format!("samples_lost: {}, recoverable: false", samples)),
        }
    }

    pub fn channel_send_failed(message: String) -> Self {
        AudioError::Channel {
            message,
            error_type: ChannelErrorType::SendFailed,
        }
    }
}

/// Helper function to create error context
pub fn create_error_context(
    component: &str,
    operation: &str,
    device_info: Option<DeviceErrorInfo>,
) -> ErrorContext {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    
    ErrorContext {
        component: component.to_string(),
        operation: operation.to_string(),
        timestamp: now,
        device_info,
        system_info: SystemErrorInfo {
            memory_usage_mb: get_memory_usage_mb(),
            cpu_usage_percent: 0.0, // Could implement CPU monitoring
            active_streams: 0, // Could track this
            buffer_utilization: 0.0, // Could track this
        },
        recovery_info: None,
    }
}

/// Get system memory usage (simplified implementation)
fn get_memory_usage_mb() -> u64 {
    // This is a simplified implementation
    // In a real system, you might use system APIs to get actual memory usage
    std::process::id() as u64 // Placeholder
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_error_handler_basic() {
        let handler = ErrorHandler::new();
        
        let error = AudioError::device_disconnected("test_device");
        let context = create_error_context("audio_stream", "connect", None);
        
        let action = handler.handle_error(error, context).await;
        
        match action {
            ErrorRecoveryAction::Retry { .. } => {
                // Expected for device errors
            }
            _ => panic!("Unexpected recovery action"),
        }
    }

    #[tokio::test]
    async fn test_error_count_tracking() {
        let handler = ErrorHandler::new();
        
        // Generate multiple errors for same component
        for i in 0..5 {
            let error = AudioError::channel_closed(&format!("test_channel_{}", i));
            let context = create_error_context("test_component", "send", None);
            handler.handle_error(error, context).await;
        }
        
        let error_count = handler.get_error_count("test_component").await;
        assert_eq!(error_count, 5);
    }

    #[tokio::test]
    async fn test_recovery_strategy_override() {
        let handler = ErrorHandler::new();
        
        // Set custom recovery strategy
        handler.set_recovery_strategy(
            "test_component".to_string(),
            ErrorRecoveryStrategy::Stop,
        ).await;
        
        let error = AudioError::buffer_overflow(1000, 1500);
        let context = create_error_context("test_component", "push", None);
        
        let action = handler.handle_error(error, context).await;
        
        match action {
            ErrorRecoveryAction::Stop => {
                // Expected with Stop strategy
            }
            _ => panic!("Unexpected recovery action"),
        }
    }

    #[tokio::test]
    async fn test_error_statistics() {
        let handler = ErrorHandler::new();
        
        // Generate errors for different components
        for component in ["comp1", "comp2", "comp3"] {
            for i in 0..3 {
                let error = AudioError::channel_closed(&format!("{}_{}", component, i));
                let context = create_error_context(component, "test", None);
                handler.handle_error(error, context).await;
            }
        }
        
        let stats = handler.get_error_statistics().await;
        assert_eq!(stats.total_errors, 9);
        assert_eq!(stats.component_errors.len(), 3);
    }
}