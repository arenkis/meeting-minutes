use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use serde::{Serialize, Deserialize};
use log::{debug, info, warn, error};

/// Strategy for handling buffer overflow situations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OverflowStrategy {
    /// Drop oldest data when buffer is full
    DropOldest,
    /// Apply backpressure to slow down producers
    Backpressure,
    /// Expand buffer up to maximum size
    Expand,
}

/// Metrics for buffer performance monitoring
#[derive(Debug, Clone, Serialize)]
pub struct BufferMetrics {
    pub current_size: usize,
    pub max_size_reached: usize,
    pub total_writes: u64,
    pub total_overflow_events: u64,
    pub average_utilization: f32,
    pub last_resize_time: Option<u64>, // timestamp in milliseconds
    pub resize_count: u64,
}

impl BufferMetrics {
    fn new() -> Self {
        Self {
            current_size: 0,
            max_size_reached: 0,
            total_writes: 0,
            total_overflow_events: 0,
            average_utilization: 0.0,
            last_resize_time: None,
            resize_count: 0,
        }
    }
}

/// Adaptive buffer that automatically adjusts its size based on load
pub struct AdaptiveBuffer<T> {
    min_size: usize,
    max_size: usize,
    current_capacity: AtomicUsize,
    data: Arc<RwLock<Vec<T>>>,
    overflow_strategy: OverflowStrategy,
    metrics: Arc<Mutex<BufferMetrics>>,
    load_tracker: LoadTracker,
    auto_resize: bool,
}

impl<T: Clone + Send + Sync> AdaptiveBuffer<T> {
    /// Create new adaptive buffer with specified size constraints
    pub fn new(min_size: usize, max_size: usize) -> Self {
        assert!(min_size <= max_size, "min_size must be <= max_size");
        
        let initial_capacity = min_size.max(1000); // Start with reasonable default
        
        Self {
            min_size,
            max_size,
            current_capacity: AtomicUsize::new(initial_capacity),
            data: Arc::new(RwLock::new(Vec::with_capacity(initial_capacity))),
            overflow_strategy: OverflowStrategy::DropOldest,
            metrics: Arc::new(Mutex::new(BufferMetrics::new())),
            load_tracker: LoadTracker::new(),
            auto_resize: true,
        }
    }

    /// Create buffer with specific overflow strategy
    pub fn with_overflow_strategy(min_size: usize, max_size: usize, strategy: OverflowStrategy) -> Self {
        let mut buffer = Self::new(min_size, max_size);
        buffer.overflow_strategy = strategy;
        buffer
    }

    /// Add item to buffer with adaptive behavior
    pub async fn push(&self, item: T) -> Result<(), BufferError> {
        let mut data = self.data.write().await;
        let current_capacity = self.current_capacity.load(Ordering::Acquire);
        
        // Update metrics
        {
            let mut metrics = self.metrics.lock().await;
            metrics.total_writes += 1;
            metrics.current_size = data.len();
            if data.len() > metrics.max_size_reached {
                metrics.max_size_reached = data.len();
            }
        }

        // Check if we need to handle overflow
        if data.len() >= current_capacity {
            match self.overflow_strategy {
                OverflowStrategy::DropOldest => {
                    if !data.is_empty() {
                        data.remove(0); // Remove oldest item
                        warn!("Buffer overflow: dropped oldest item (capacity: {})", current_capacity);
                        
                        let mut metrics = self.metrics.lock().await;
                        metrics.total_overflow_events += 1;
                    }
                }
                OverflowStrategy::Backpressure => {
                    warn!("Buffer full, applying backpressure (capacity: {})", current_capacity);
                    return Err(BufferError::BufferFull);
                }
                OverflowStrategy::Expand => {
                    if current_capacity < self.max_size {
                        let new_capacity = (current_capacity * 2).min(self.max_size);
                        self.resize_buffer(new_capacity).await?;
                        info!("Buffer expanded from {} to {}", current_capacity, new_capacity);
                    } else {
                        // Max size reached, fall back to drop oldest
                        if !data.is_empty() {
                            data.remove(0);
                            warn!("Max buffer size reached, dropping oldest item");
                        }
                    }
                }
            }
        }

        data.push(item);
        
        // Update load tracking
        self.load_tracker.record_write();
        
        // Check if we should auto-resize
        if self.auto_resize {
            self.check_and_adjust_capacity().await;
        }

        Ok(())
    }

    /// Remove and return the oldest item from buffer
    pub async fn pop(&self) -> Option<T> {
        let mut data = self.data.write().await;
        let item = if !data.is_empty() {
            Some(data.remove(0))
        } else {
            None
        };

        // Update load tracking
        self.load_tracker.record_read();
        
        // Check if we should auto-resize
        if self.auto_resize {
            self.check_and_adjust_capacity().await;
        }

        item
    }

    /// Get current buffer utilization (0.0 to 1.0)
    pub async fn utilization(&self) -> f32 {
        let data = self.data.read().await;
        let current_capacity = self.current_capacity.load(Ordering::Acquire);
        data.len() as f32 / current_capacity as f32
    }

    /// Get current buffer length
    pub async fn len(&self) -> usize {
        let data = self.data.read().await;
        data.len()
    }

    /// Check if buffer is empty
    pub async fn is_empty(&self) -> bool {
        let data = self.data.read().await;
        data.is_empty()
    }

    /// Get buffer metrics
    pub async fn metrics(&self) -> BufferMetrics {
        let mut metrics = self.metrics.lock().await;
        let utilization = self.utilization().await;
        metrics.average_utilization = (metrics.average_utilization * 0.9) + (utilization * 0.1);
        metrics.clone()
    }

    /// Manually adjust buffer capacity
    pub async fn adjust_capacity(&self, load_factor: f32) {
        let current_capacity = self.current_capacity.load(Ordering::Acquire);
        
        let new_capacity = if load_factor > 0.8 {
            // High load, expand buffer
            ((current_capacity as f32 * 1.5) as usize).min(self.max_size)
        } else if load_factor < 0.3 {
            // Low load, shrink buffer
            ((current_capacity as f32 * 0.75) as usize).max(self.min_size)
        } else {
            // Moderate load, keep current size
            current_capacity
        };

        if new_capacity != current_capacity {
            if let Err(e) = self.resize_buffer(new_capacity).await {
                warn!("Failed to resize buffer: {}", e);
            } else {
                info!("Buffer capacity adjusted from {} to {} (load factor: {:.2})", 
                     current_capacity, new_capacity, load_factor);
            }
        }
    }

    /// Check and adjust capacity based on current load
    async fn check_and_adjust_capacity(&self) {
        let load_factor = self.load_tracker.current_load();
        
        // Only adjust if load factor is significantly different from optimal
        if load_factor > 0.85 || load_factor < 0.25 {
            self.adjust_capacity(load_factor).await;
        }
    }

    /// Resize the internal buffer
    async fn resize_buffer(&self, new_capacity: usize) -> Result<(), BufferError> {
        let mut data = self.data.write().await;
        
        if new_capacity < data.len() {
            return Err(BufferError::CapacityTooSmall);
        }

        // Reserve new capacity
        let current_capacity = data.capacity();
        data.reserve(new_capacity.saturating_sub(current_capacity));
        
        // Update capacity
        self.current_capacity.store(new_capacity, Ordering::Release);
        
        // Update metrics
        {
            let mut metrics = self.metrics.lock().await;
            metrics.last_resize_time = Some(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64);
            metrics.resize_count += 1;
        }

        Ok(())
    }

    /// Clear all items from buffer
    pub async fn clear(&self) {
        let mut data = self.data.write().await;
        data.clear();
        
        // Reset to minimum size for efficiency
        let min_capacity = self.min_size;
        self.current_capacity.store(min_capacity, Ordering::Release);
        data.shrink_to(min_capacity);
    }

    /// Get current buffer capacity
    pub fn current_capacity(&self) -> usize {
        self.current_capacity.load(Ordering::Acquire)
    }
}

/// Load tracker for monitoring buffer usage patterns
struct LoadTracker {
    write_count: AtomicU64,
    read_count: AtomicU64,
    last_measurement: Arc<Mutex<Instant>>,
}

impl LoadTracker {
    fn new() -> Self {
        Self {
            write_count: AtomicU64::new(0),
            read_count: AtomicU64::new(0),
            last_measurement: Arc::new(Mutex::new(Instant::now())),
        }
    }

    fn record_write(&self) {
        self.write_count.fetch_add(1, Ordering::Relaxed);
    }

    fn record_read(&self) {
        self.read_count.fetch_add(1, Ordering::Relaxed);
    }

    fn current_load(&self) -> f32 {
        let writes = self.write_count.load(Ordering::Relaxed);
        let reads = self.read_count.load(Ordering::Relaxed);
        
        if reads == 0 && writes == 0 {
            return 0.0;
        }
        
        // Calculate load based on write/read ratio
        // High write rate vs read rate = high load
        let total_ops = writes + reads;
        if total_ops == 0 {
            0.0
        } else {
            writes as f32 / total_ops as f32
        }
    }
}

/// Errors that can occur during buffer operations
#[derive(Debug, thiserror::Error)]
pub enum BufferError {
    #[error("Buffer is full and cannot accept more items")]
    BufferFull,
    #[error("Requested capacity is too small for current data")]
    CapacityTooSmall,
    #[error("Buffer operation failed: {0}")]
    OperationFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_adaptive_buffer_basic_operations() {
        let buffer = AdaptiveBuffer::new(10, 100);
        
        // Test push and pop
        buffer.push(1).await.unwrap();
        buffer.push(2).await.unwrap();
        
        assert_eq!(buffer.len().await, 2);
        assert_eq!(buffer.pop().await, Some(1));
        assert_eq!(buffer.pop().await, Some(2));
        assert!(buffer.is_empty().await);
    }

    #[tokio::test]
    async fn test_buffer_overflow_drop_oldest() {
        let buffer = AdaptiveBuffer::with_overflow_strategy(2, 2, OverflowStrategy::DropOldest);
        
        // Fill buffer
        buffer.push(1).await.unwrap();
        buffer.push(2).await.unwrap();
        
        // This should drop the oldest item (1)
        buffer.push(3).await.unwrap();
        
        assert_eq!(buffer.len().await, 2);
        assert_eq!(buffer.pop().await, Some(2));
        assert_eq!(buffer.pop().await, Some(3));
    }

    #[tokio::test]
    async fn test_buffer_expansion() {
        let buffer = AdaptiveBuffer::with_overflow_strategy(2, 10, OverflowStrategy::Expand);
        
        // Fill initial capacity
        buffer.push(1).await.unwrap();
        buffer.push(2).await.unwrap();
        
        // This should trigger expansion
        buffer.push(3).await.unwrap();
        
        assert_eq!(buffer.len().await, 3);
        assert!(buffer.current_capacity.load(Ordering::Acquire) > 2);
    }

    #[tokio::test]
    async fn test_load_tracking_and_adjustment() {
        let buffer = AdaptiveBuffer::new(10, 100);
        
        // Simulate high load
        for i in 0..20 {
            buffer.push(i).await.unwrap();
        }
        
        // Check if buffer expanded due to high load
        let capacity = buffer.current_capacity.load(Ordering::Acquire);
        assert!(capacity >= 10);
        
        // Simulate low load by reading most items
        for _ in 0..15 {
            buffer.pop().await;
        }
        
        // Allow some time for auto-adjustment
        sleep(Duration::from_millis(10)).await;
        
        let metrics = buffer.metrics().await;
        assert!(metrics.total_writes > 0);
    }
}