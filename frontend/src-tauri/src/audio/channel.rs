use std::sync::atomic::{AtomicU32, AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock, broadcast, mpsc};
use tokio::time::timeout;
use serde::{Serialize, Deserialize};
use anyhow::{Result, anyhow};
use log::{debug, info, warn, error};

use super::buffer::AdaptiveBuffer;
use super::error::{AudioError, ErrorHandler, create_error_context};

/// Channel state for tracking connection health
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChannelState {
    /// Channel is active and functioning normally
    Active,
    /// Channel is temporarily disconnected but attempting recovery
    Recovering,
    /// Channel has failed and requires manual intervention
    Failed,
    /// Channel has been deliberately closed
    Closed,
    /// Channel is being initialized
    Initializing,
}

/// Recovery strategy for failed channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    /// Exponential backoff with configurable parameters
    ExponentialBackoff {
        base_delay_ms: u64,
        max_delay_ms: u64,
        max_retries: u32,
    },
    /// Fixed delay between retries
    FixedDelay {
        delay_ms: u64,
        max_retries: u32,
    },
    /// No automatic recovery
    None,
}

/// Health monitoring for channels
pub struct HealthMonitor {
    last_activity: AtomicU64,
    error_count: AtomicU32,
    recovery_attempts: AtomicU32,
    last_recovery_attempt: AtomicU64,
    is_healthy: AtomicBool,
}

impl HealthMonitor {
    pub fn new() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        Self {
            last_activity: AtomicU64::new(now),
            error_count: AtomicU32::new(0),
            recovery_attempts: AtomicU32::new(0),
            last_recovery_attempt: AtomicU64::new(0),
            is_healthy: AtomicBool::new(true),
        }
    }

    pub fn record_activity(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        
        self.last_activity.store(now, Ordering::Relaxed);
        self.is_healthy.store(true, Ordering::Relaxed);
        
        // Reset error count on successful activity
        if self.error_count.load(Ordering::Relaxed) > 0 {
            info!("Channel healthy again, resetting error count");
            self.error_count.store(0, Ordering::Relaxed);
        }
    }

    pub fn record_error(&self) {
        let error_count = self.error_count.fetch_add(1, Ordering::Relaxed) + 1;
        warn!("Channel error recorded, count: {}", error_count);
        
        // Mark as unhealthy after 3 errors
        if error_count >= 3 {
            self.is_healthy.store(false, Ordering::Relaxed);
            warn!("Channel marked as unhealthy due to repeated errors");
        }
    }

    pub fn record_recovery_attempt(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        let attempt_count = self.recovery_attempts.fetch_add(1, Ordering::Relaxed) + 1;
        self.last_recovery_attempt.store(now, Ordering::Relaxed);
        
        info!("Recovery attempt #{} initiated", attempt_count);
    }

    pub fn is_healthy(&self) -> bool {
        self.is_healthy.load(Ordering::Relaxed)
    }

    pub fn time_since_last_activity(&self) -> Duration {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last = self.last_activity.load(Ordering::Relaxed);
        
        Duration::from_millis(now.saturating_sub(last))
    }

    pub fn should_attempt_recovery(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last_attempt = self.last_recovery_attempt.load(Ordering::Relaxed);
        let attempts = self.recovery_attempts.load(Ordering::Relaxed);
        
        // Don't attempt recovery if we've tried too many times recently
        if attempts > 10 {
            return false;
        }
        
        // Exponential backoff: wait longer between attempts
        let backoff_duration = 2_u64.pow(attempts.min(10)) * 1000; // Milliseconds
        (now - last_attempt) > backoff_duration
    }
}

/// Channel health metrics for monitoring
#[derive(Debug, Clone, Serialize)]
pub struct ChannelHealthMetrics {
    pub state: ChannelState,
    pub is_healthy: bool,
    pub error_count: u32,
    pub recovery_attempts: u32,
    pub time_since_last_activity_ms: u64,
}

/// Managed channel with recovery capabilities
pub struct ManagedChannel<T> {
    sender: Arc<Mutex<Option<broadcast::Sender<T>>>>,
    state: Arc<RwLock<ChannelState>>,
    health_monitor: Arc<HealthMonitor>,
    recovery_strategy: RecoveryStrategy,
    buffer: Arc<AdaptiveBuffer<T>>,
    channel_id: String,
    error_handler: Arc<ErrorHandler>,
}

impl<T> ManagedChannel<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new(capacity: usize, recovery_strategy: RecoveryStrategy, channel_id: String) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        
        Self {
            sender: Arc::new(Mutex::new(Some(tx))),
            state: Arc::new(RwLock::new(ChannelState::Initializing)),
            health_monitor: Arc::new(HealthMonitor::new()),
            recovery_strategy,
            buffer: Arc::new(AdaptiveBuffer::with_overflow_strategy(
                capacity, 
                capacity * 2, 
                super::buffer::OverflowStrategy::DropOldest
            )),
            channel_id,
            error_handler: Arc::new(ErrorHandler::new()),
        }
    }

    pub async fn send(&self, data: T) -> Result<()> {
        let sender_lock = self.sender.lock().await;
        if let Some(ref sender) = *sender_lock {
            match sender.send(data.clone()) {
                Ok(_) => {
                    self.health_monitor.record_activity();
                    *self.state.write().await = ChannelState::Active;
                    Ok(())
                }
                Err(_) => {
                    // Channel has no receivers, buffer the data
                    self.buffer.push(data).await.map_err(|e| anyhow!("Buffer failed: {}", e))
                }
            }
        } else {
            Err(anyhow!("Channel is closed"))
        }
    }

    pub async fn subscribe(&self) -> Result<broadcast::Receiver<T>> {
        let sender_lock = self.sender.lock().await;
        if let Some(ref sender) = *sender_lock {
            Ok(sender.subscribe())
        } else {
            Err(anyhow!("Channel is closed"))
        }
    }

    pub async fn get_health(&self) -> ChannelHealthMetrics {
        let state = self.state.read().await.clone();
        let is_healthy = self.health_monitor.is_healthy();
        let error_count = self.health_monitor.error_count.load(Ordering::Relaxed);
        let recovery_attempts = self.health_monitor.recovery_attempts.load(Ordering::Relaxed);
        let time_since_last_activity_ms = self.health_monitor.time_since_last_activity().as_millis() as u64;

        ChannelHealthMetrics {
            state,
            is_healthy,
            error_count,
            recovery_attempts,
            time_since_last_activity_ms,
        }
    }

    /// Close the channel
    pub async fn close(&self) -> Result<()> {
        let mut sender_lock = self.sender.lock().await;
        *sender_lock = None;
        *self.state.write().await = ChannelState::Closed;
        info!("Channel {} closed", self.channel_id);
        Ok(())
    }

    /// Initiate recovery for a failed channel
    pub async fn initiate_recovery(&self) -> Result<()> {
        if !self.health_monitor.should_attempt_recovery() {
            return Err(anyhow!("Recovery not needed or too early"));
        }

        self.health_monitor.record_recovery_attempt();
        *self.state.write().await = ChannelState::Recovering;

        // Create new channel
        let capacity = self.buffer.current_capacity();
        let (tx, _) = broadcast::channel(capacity);
        
        {
            let mut sender_lock = self.sender.lock().await;
            *sender_lock = Some(tx);
        }

        *self.state.write().await = ChannelState::Active;
        info!("Channel {} recovery initiated", self.channel_id);
        Ok(())
    }

    /// Send with backpressure handling - attempts regular send first, then buffers
    pub async fn send_with_backpressure(&self, data: T) -> Result<()> {
        // Try regular send first
        match self.send(data.clone()).await {
            Ok(_) => Ok(()),
            Err(_) => {
                // If send fails, try to buffer the data
                self.buffer.push(data).await.map_err(|e| anyhow!("Failed to buffer data: {}", e))
            }
        }
    }

    /// Get detailed health metrics
    pub async fn health_metrics(&self) -> ChannelHealthMetrics {
        self.get_health().await
    }

    /// Check if channel is healthy
    pub async fn is_healthy(&self) -> bool {
        self.get_health().await.is_healthy
    }
}

