use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex, mpsc, broadcast};
use tokio::task::JoinHandle;
use serde::{Serialize, Deserialize};
use anyhow::{Result, anyhow};
use log::{debug, info, warn, error};

use super::{
    AudioDevice, ManagedChannel, ChannelState, DualChannelVad, DualChannelVadStats,
    StreamingWhisperService, StreamingWhisperConfig, StreamingTranscriptionResult,
    IntelligentChunker, ChunkingConfig, BoundaryType,
    AudioError, ErrorHandler, ErrorRecoveryAction, create_error_context,
};
use crate::whisper_engine::{WhisperEngine, ModelInfo};

/// Configuration for the context manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManagerConfig {
    /// Sample rate for all audio processing
    pub sample_rate: usize,
    /// Buffer size for audio chunks
    pub buffer_size_ms: u32,
    /// Maximum context history to maintain in seconds
    pub max_context_duration_s: u32,
    /// Minimum chunk size before processing
    pub min_chunk_size_ms: u32,
    /// Maximum chunk size before forced processing
    pub max_chunk_size_ms: u32,
    /// Timeout for processing individual chunks
    pub chunk_timeout_ms: u64,
    /// Enable automatic model management
    pub auto_model_management: bool,
    /// Preferred whisper model
    pub preferred_model: String,
    /// Enable context persistence across sessions
    pub persist_context: bool,
}

impl Default for ContextManagerConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            buffer_size_ms: 100, // 100ms buffers for responsive processing
            max_context_duration_s: 300, // 5 minutes of context
            min_chunk_size_ms: 1000, // 1 second minimum
            max_chunk_size_ms: 30000, // 30 seconds maximum  
            chunk_timeout_ms: 10000, // 10 seconds timeout
            auto_model_management: true,
            preferred_model: "base".to_string(),
            persist_context: true,
        }
    }
}

/// Audio input source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSourceConfig {
    pub device: AudioDevice,
    pub enabled: bool,
    pub gain: f32,
    pub channel_name: String,
}

/// Transcription result with enhanced metadata
#[derive(Debug, Clone, Serialize)]
pub struct EnhancedTranscriptionResult {
    /// Core transcription result
    pub transcription: StreamingTranscriptionResult,
    /// Source device information
    pub source: String,
    /// Global sequence ID for ordering
    pub sequence_id: u64,
    /// Processing metadata
    pub metadata: TranscriptionMetadata,
}

/// Metadata about transcription processing
#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionMetadata {
    pub audio_samples: usize,
    pub vad_stats: Option<DualChannelVadStats>,
    pub chunk_boundary: BoundaryType,
    pub processing_chain: Vec<String>,
    pub total_latency_ms: u64,
    pub audio_received_at: std::time::SystemTime,
    pub transcription_completed_at: std::time::SystemTime,
}

/// Current status of the context manager
#[derive(Debug, Clone, Serialize)]
pub struct ContextManagerStatus {
    pub is_active: bool,
    pub current_model: Option<String>,
    pub audio_sources: Vec<AudioSourceStatus>,
    pub processing_stats: ProcessingStats,
    pub error_count: u64,
    pub uptime_ms: u64,
}

/// Status of individual audio source
#[derive(Debug, Clone, Serialize)]
pub struct AudioSourceStatus {
    pub name: String,
    pub is_active: bool,
    pub samples_processed: u64,
    pub last_activity: Option<std::time::SystemTime>,
    pub channel_health: ChannelState,
}

/// Processing performance statistics
#[derive(Debug, Clone, Serialize)]
pub struct ProcessingStats {
    pub total_transcriptions: u64,
    pub average_latency_ms: f64,
    pub chunks_processed: u64,
    pub vad_efficiency: f32, // percentage of audio determined to be speech
    pub context_hit_rate: f32, // percentage of transcriptions that used context
    pub error_rate: f32, // percentage of failed processing attempts
}

/// Events emitted by the context manager
#[derive(Debug, Clone, Serialize)]
pub enum ContextManagerEvent {
    /// New transcription available
    TranscriptionReady(EnhancedTranscriptionResult),
    /// Audio source status changed
    AudioSourceChanged { source: String, active: bool },
    /// Model changed
    ModelChanged { old_model: Option<String>, new_model: String },
    /// Processing error occurred
    ProcessingError { error: String, source: String, recoverable: bool },
    /// Context manager status update
    StatusUpdate(ContextManagerStatus),
}

/// Central orchestrator for streaming transcription pipeline
pub struct StreamingTranscriptionContextManager {
    /// Configuration
    config: ContextManagerConfig,
    
    /// Audio source management
    mic_channel: Arc<ManagedChannel<Vec<f32>>>,
    speaker_channel: Arc<ManagedChannel<Vec<f32>>>,
    
    /// Processing components
    vad_processor: Arc<Mutex<DualChannelVad>>,
    whisper_service: Arc<StreamingWhisperService>,
    whisper_engine: Arc<WhisperEngine>,
    
    /// Event broadcasting
    event_broadcaster: broadcast::Sender<ContextManagerEvent>,
    
    /// Processing task handles
    processing_tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
    
    /// Statistics and monitoring
    stats: Arc<RwLock<ProcessingStats>>,
    start_time: Instant,
    sequence_counter: Arc<std::sync::atomic::AtomicU64>,
    
    /// Error handling
    error_handler: Arc<ErrorHandler>,
    
    /// State management
    is_active: Arc<std::sync::atomic::AtomicBool>,
    current_model: Arc<RwLock<Option<String>>>,
}

impl StreamingTranscriptionContextManager {
    /// Create new context manager
    pub async fn new(config: ContextManagerConfig) -> Result<Self> {
        info!("Initializing StreamingTranscriptionContextManager");

        // Create audio channels
        let mic_channel = Arc::new(ManagedChannel::new(
            1000,
            super::RecoveryStrategy::ExponentialBackoff { 
                base_delay_ms: 100, 
                max_delay_ms: 5000, 
                max_retries: 5 
            },
            "microphone".to_string(),
        ));

        let speaker_channel = Arc::new(ManagedChannel::new(
            1000,
            super::RecoveryStrategy::ExponentialBackoff { 
                base_delay_ms: 100, 
                max_delay_ms: 5000, 
                max_retries: 5 
            },
            "speaker".to_string(),
        ));

        // Create VAD processor
        let vad_processor = Arc::new(Mutex::new(
            DualChannelVad::new(config.sample_rate)?
        ));

        // Create whisper components
        let whisper_engine = Arc::new(WhisperEngine::new()?);
        
        let whisper_config = StreamingWhisperConfig {
            sample_rate: config.sample_rate,
            max_context_samples: config.sample_rate * config.max_context_duration_s as usize,
            context_overlap_samples: config.sample_rate / 10, // 100ms overlap
            max_retries: 3,
            base_temperature: 0.0,
            temperature_increment: 0.2,
            max_temperature: 1.0,
            language: Some("en".to_string()),
            enable_timestamps: true,
            confidence_threshold: 0.3,
            max_processing_time_ms: config.chunk_timeout_ms,
        };
        
        let whisper_service = Arc::new(StreamingWhisperService::new(whisper_config)?);

        // Create event broadcaster
        let (event_sender, _) = broadcast::channel(1000);

        let manager = Self {
            config,
            mic_channel,
            speaker_channel,
            vad_processor,
            whisper_service,
            whisper_engine,
            event_broadcaster: event_sender,
            processing_tasks: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(RwLock::new(ProcessingStats {
                total_transcriptions: 0,
                average_latency_ms: 0.0,
                chunks_processed: 0,
                vad_efficiency: 0.0,
                context_hit_rate: 0.0,
                error_rate: 0.0,
            })),
            start_time: Instant::now(),
            sequence_counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            error_handler: Arc::new(ErrorHandler::new()),
            is_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            current_model: Arc::new(RwLock::new(None)),
        };

        // Auto-load preferred model if enabled
        if manager.config.auto_model_management {
            manager.ensure_model_loaded().await?;
        }

        info!("StreamingTranscriptionContextManager initialized successfully");
        Ok(manager)
    }

    /// Start the transcription pipeline
    pub async fn start(&self) -> Result<()> {
        if self.is_active.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(anyhow!("Context manager is already active"));
        }

        info!("Starting streaming transcription pipeline");

        // Ensure model is loaded
        self.ensure_model_loaded().await?;

        // Start processing tasks
        self.start_processing_pipeline().await?;

        // Mark as active
        self.is_active.store(true, std::sync::atomic::Ordering::Relaxed);

        // Emit status update
        let status = self.get_status().await;
        let _ = self.event_broadcaster.send(ContextManagerEvent::StatusUpdate(status));

        info!("Streaming transcription pipeline started successfully");
        Ok(())
    }

    /// Stop the transcription pipeline
    pub async fn stop(&self) -> Result<()> {
        if !self.is_active.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(()); // Already stopped
        }

        info!("Stopping streaming transcription pipeline");

        // Mark as inactive
        self.is_active.store(false, std::sync::atomic::Ordering::Relaxed);

        // Stop all processing tasks
        {
            let mut tasks = self.processing_tasks.lock().await;
            for task in tasks.drain(..) {
                task.abort();
                let _ = task.await; // Ignore cancellation errors
            }
        }

        // Reset whisper service context
        self.whisper_service.reset_context().await;

        // Emit status update
        let status = self.get_status().await;
        let _ = self.event_broadcaster.send(ContextManagerEvent::StatusUpdate(status));

        info!("Streaming transcription pipeline stopped");
        Ok(())
    }

    /// Ensure whisper model is loaded
    async fn ensure_model_loaded(&self) -> Result<()> {
        let current_model = self.current_model.read().await.clone();
        
        if current_model.is_none() {
            info!("Loading preferred whisper model: {}", self.config.preferred_model);
            
            // Discover available models
            let models = self.whisper_engine.discover_models().await?;
            let target_model = models.iter()
                .find(|m| m.name == self.config.preferred_model)
                .ok_or_else(|| anyhow!("Preferred model '{}' not found", self.config.preferred_model))?;

            // Load the model
            match &target_model.status {
                crate::whisper_engine::ModelStatus::Available => {
                    self.whisper_engine.load_model(&self.config.preferred_model).await?;
                }
                crate::whisper_engine::ModelStatus::Missing => {
                    return Err(anyhow!("Model '{}' needs to be downloaded first", self.config.preferred_model));
                }
                _ => {
                    return Err(anyhow!("Model '{}' is not ready for use", self.config.preferred_model));
                }
            }

            // Initialize whisper service with the loaded context  
            // Note: We need access to the whisper context from WhisperEngine
            // This requires modification to WhisperEngine to expose the context
            info!("Whisper model '{}' loaded successfully", self.config.preferred_model);
            
            *self.current_model.write().await = Some(self.config.preferred_model.clone());
            
            let _ = self.event_broadcaster.send(ContextManagerEvent::ModelChanged {
                old_model: current_model,
                new_model: self.config.preferred_model.clone(),
            });
        }

        Ok(())
    }

    /// Start the processing pipeline tasks
    async fn start_processing_pipeline(&self) -> Result<()> {
        let mut tasks = self.processing_tasks.lock().await;

        // Task 1: Process microphone audio
        {
            let mic_channel = Arc::clone(&self.mic_channel);
            let vad_processor = Arc::clone(&self.vad_processor);
            let whisper_service = Arc::clone(&self.whisper_service);
            let event_sender = self.event_broadcaster.clone();
            let is_active = Arc::clone(&self.is_active);
            let stats = Arc::clone(&self.stats);
            let sequence_counter = Arc::clone(&self.sequence_counter);
            let error_handler = Arc::clone(&self.error_handler);

            let task = tokio::spawn(async move {
                Self::process_audio_stream(
                    mic_channel,
                    "microphone".to_string(),
                    vad_processor,
                    whisper_service,
                    event_sender,
                    is_active,
                    stats,
                    sequence_counter,
                    error_handler,
                ).await;
            });

            tasks.push(task);
        }

        // Task 2: Process speaker audio (similar structure)
        {
            let speaker_channel = Arc::clone(&self.speaker_channel);
            let vad_processor = Arc::clone(&self.vad_processor);
            let whisper_service = Arc::clone(&self.whisper_service);
            let event_sender = self.event_broadcaster.clone();
            let is_active = Arc::clone(&self.is_active);
            let stats = Arc::clone(&self.stats);
            let sequence_counter = Arc::clone(&self.sequence_counter);
            let error_handler = Arc::clone(&self.error_handler);

            let task = tokio::spawn(async move {
                Self::process_audio_stream(
                    speaker_channel,
                    "speaker".to_string(),
                    vad_processor,
                    whisper_service,
                    event_sender,
                    is_active,
                    stats,
                    sequence_counter,
                    error_handler,
                ).await;
            });

            tasks.push(task);
        }

        info!("Started {} processing tasks", tasks.len());
        Ok(())
    }

    /// Process audio stream from a channel
    async fn process_audio_stream(
        channel: Arc<ManagedChannel<Vec<f32>>>,
        source_name: String,
        vad_processor: Arc<Mutex<DualChannelVad>>,
        whisper_service: Arc<StreamingWhisperService>,
        event_sender: broadcast::Sender<ContextManagerEvent>,
        is_active: Arc<std::sync::atomic::AtomicBool>,
        stats: Arc<RwLock<ProcessingStats>>,
        sequence_counter: Arc<std::sync::atomic::AtomicU64>,
        error_handler: Arc<ErrorHandler>,
    ) {
        info!("Starting audio processing for source: {}", source_name);

        let mut receiver = match channel.subscribe().await {
            Ok(rx) => rx,
            Err(e) => {
                error!("Failed to subscribe to {} channel: {}", source_name, e);
                return;
            }
        };

        while is_active.load(std::sync::atomic::Ordering::Relaxed) {
            match receiver.recv().await {
                Ok(audio_samples) => {
                    let processing_start = Instant::now();
                    let audio_received_at = std::time::SystemTime::now();

                    debug!("Processing {} samples from {}", audio_samples.len(), source_name);

                    // Process through streaming pipeline
                    match whisper_service.process_streaming_audio(&audio_samples).await {
                        Ok(transcription_results) => {
                            let transcription_completed_at = std::time::SystemTime::now();
                            let total_latency = processing_start.elapsed().as_millis() as u64;

                            // Process each transcription result
                            for transcription in transcription_results {
                                let sequence_id = sequence_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                
                                let enhanced_result = EnhancedTranscriptionResult {
                                    transcription,
                                    source: source_name.clone(),
                                    sequence_id,
                                    metadata: TranscriptionMetadata {
                                        audio_samples: audio_samples.len(),
                                        vad_stats: None, // Could be populated if needed
                                        chunk_boundary: BoundaryType::SpeechEnd, // From transcription result
                                        processing_chain: vec!["streaming_vad".to_string(), "intelligent_chunking".to_string(), "streaming_whisper".to_string()],
                                        total_latency_ms: total_latency,
                                        audio_received_at,
                                        transcription_completed_at,
                                    },
                                };

                                // Update statistics
                                {
                                    let mut stats_guard = stats.write().await;
                                    stats_guard.total_transcriptions += 1;
                                    stats_guard.chunks_processed += 1;
                                    
                                    // Update average latency
                                    let total_latency_ms = stats_guard.average_latency_ms * (stats_guard.total_transcriptions - 1) as f64 + total_latency as f64;
                                    stats_guard.average_latency_ms = total_latency_ms / stats_guard.total_transcriptions as f64;

                                    // Update context hit rate
                                    if enhanced_result.transcription.has_context {
                                        stats_guard.context_hit_rate = (stats_guard.context_hit_rate * (stats_guard.total_transcriptions - 1) as f32 + 1.0) / stats_guard.total_transcriptions as f32;
                                    } else {
                                        stats_guard.context_hit_rate = stats_guard.context_hit_rate * (stats_guard.total_transcriptions - 1) as f32 / stats_guard.total_transcriptions as f32;
                                    }
                                }

                                // Emit transcription event
                                if !enhanced_result.transcription.text.trim().is_empty() {
                                    let _ = event_sender.send(ContextManagerEvent::TranscriptionReady(enhanced_result));
                                }
                            }
                        }
                        Err(e) => {
                            let error = AudioError::transcription_failed(audio_samples.len(), &e.to_string());
                            let context = create_error_context("context_manager", &format!("process_{}", source_name), None);
                            let action = error_handler.handle_error(error, context).await;

                            // Update error statistics
                            {
                                let mut stats_guard = stats.write().await;
                                stats_guard.chunks_processed += 1;
                                stats_guard.error_rate = (stats_guard.error_rate * (stats_guard.chunks_processed - 1) as f32 + 1.0) / stats_guard.chunks_processed as f32;
                            }

                            let recoverable = matches!(action, ErrorRecoveryAction::Retry { .. } | ErrorRecoveryAction::Backoff { .. });
                            let _ = event_sender.send(ContextManagerEvent::ProcessingError {
                                error: e.to_string(),
                                source: source_name.clone(),
                                recoverable,
                            });

                            warn!("Transcription failed for {}: {}", source_name, e);
                        }
                    }
                }
                Err(e) => {
                    if is_active.load(std::sync::atomic::Ordering::Relaxed) {
                        warn!("Error receiving audio from {}: {}", source_name, e);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        }

        info!("Audio processing stopped for source: {}", source_name);
    }

    /// Subscribe to context manager events
    pub fn subscribe_to_events(&self) -> broadcast::Receiver<ContextManagerEvent> {
        self.event_broadcaster.subscribe()
    }

    /// Get current status
    pub async fn get_status(&self) -> ContextManagerStatus {
        let stats = self.stats.read().await.clone();
        let current_model = self.current_model.read().await.clone();
        let is_active = self.is_active.load(std::sync::atomic::Ordering::Relaxed);
        let uptime_ms = self.start_time.elapsed().as_millis() as u64;

        // Get audio source statuses
        let mic_health = self.mic_channel.get_health().await;
        let speaker_health = self.speaker_channel.get_health().await;

        let audio_sources = vec![
            AudioSourceStatus {
                name: "microphone".to_string(),
                is_active: mic_health.is_healthy,
                samples_processed: 0, // Would need to track this
                last_activity: None, // Would need to track this
                channel_health: mic_health.state,
            },
            AudioSourceStatus {
                name: "speaker".to_string(),
                is_active: speaker_health.is_healthy,
                samples_processed: 0, // Would need to track this  
                last_activity: None, // Would need to track this
                channel_health: speaker_health.state,
            },
        ];

        ContextManagerStatus {
            is_active,
            current_model,
            audio_sources,
            processing_stats: stats,
            error_count: 0, // Would need to track this
            uptime_ms,
        }
    }

    /// Get microphone channel for audio input
    pub fn get_mic_channel(&self) -> Arc<ManagedChannel<Vec<f32>>> {
        Arc::clone(&self.mic_channel)
    }

    /// Get speaker channel for audio input
    pub fn get_speaker_channel(&self) -> Arc<ManagedChannel<Vec<f32>>> {
        Arc::clone(&self.speaker_channel)
    }

    /// Get whisper service for direct access
    pub fn get_whisper_service(&self) -> Arc<StreamingWhisperService> {
        Arc::clone(&self.whisper_service)
    }

    /// Change the active whisper model
    pub async fn change_model(&self, model_name: String) -> Result<()> {
        info!("Changing whisper model to: {}", model_name);

        // Temporarily stop processing
        let was_active = self.is_active.load(std::sync::atomic::Ordering::Relaxed);
        if was_active {
            self.stop().await?;
        }

        // Load new model
        self.whisper_engine.load_model(&model_name).await?;
        
        let old_model = self.current_model.read().await.clone();
        *self.current_model.write().await = Some(model_name.clone());

        // Emit model change event
        let _ = self.event_broadcaster.send(ContextManagerEvent::ModelChanged {
            old_model,
            new_model: model_name,
        });

        // Restart if was active
        if was_active {
            self.start().await?;
        }

        Ok(())
    }

    /// Reset all transcription context
    pub async fn reset_context(&self) -> Result<()> {
        info!("Resetting transcription context");

        // Reset whisper service context
        self.whisper_service.reset_context().await;

        // Reset VAD processor
        {
            let mut vad = self.vad_processor.lock().await;
            vad.reset();
        }

        // Reset statistics
        {
            let mut stats = self.stats.write().await;
            *stats = ProcessingStats {
                total_transcriptions: 0,
                average_latency_ms: 0.0,
                chunks_processed: 0,
                vad_efficiency: 0.0,
                context_hit_rate: 0.0,
                error_rate: 0.0,
            };
        }

        info!("Transcription context reset successfully");
        Ok(())
    }
}

/// Drop implementation to ensure cleanup
impl Drop for StreamingTranscriptionContextManager {
    fn drop(&mut self) {
        // Note: Can't use async in Drop, so we just set inactive flag
        self.is_active.store(false, std::sync::atomic::Ordering::Relaxed);
        info!("StreamingTranscriptionContextManager dropped");
    }
}