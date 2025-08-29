use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{RwLock, Mutex};
use tokio_util::sync::CancellationToken;
use anyhow::Result;
use log::{info, error, debug, warn};
use serde::{Deserialize, Serialize};
use tauri::{Runtime, AppHandle, Emitter};

use super::vad::DualChannelVad;
use super::error::{AudioError, ErrorHandler};
use crate::whisper_engine::WhisperEngine;
use crate::utils::format_timestamp;

/// Configuration for the streaming service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    /// VAD Configuration
    pub chunk_duration_ms: u32,        // 30ms optimal chunks
    pub redemption_time_ms: u32,       // 500ms for speech boundary detection
    pub pre_speech_pad_ms: u32,        // 100ms context preservation
    pub min_speech_duration_ms: u32,   // 50ms minimum for processing
    
    /// Whisper Configuration
    pub model_type: String,            // tiny/base/small/medium
    pub temperature: f32,              // 0.0, increases by 0.2 for retries
    pub gpu_acceleration: bool,        // Default: true
    pub language: String,              // Default: "en"
    
    /// Performance Settings
    pub max_segment_duration_ms: u32,  // 30000ms maximum
    pub enable_vad: bool,              // Default: true
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            chunk_duration_ms: 30,
            redemption_time_ms: 500,
            pre_speech_pad_ms: 100,
            min_speech_duration_ms: 50,
            model_type: "base".to_string(),
            temperature: 0.0,
            gpu_acceleration: true,
            language: "en".to_string(),
            max_segment_duration_ms: 30000,
            enable_vad: true,
        }
    }
}

/// Connection manager for single-stream processing
pub struct ConnectionManager {
    inner: Mutex<Option<CancellationToken>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub async fn acquire_connection(&self) -> ConnectionGuard {
        let mut slot = self.inner.lock().await;
        if let Some(old) = slot.take() {
            old.cancel(); // Graceful termination of previous connection
        }
        let token = CancellationToken::new();
        *slot = Some(token.clone());
        ConnectionGuard { token }
    }
}

pub struct ConnectionGuard {
    token: CancellationToken,
}

impl ConnectionGuard {
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}

/// Transcript result with proper timestamp preservation
#[derive(Debug, Clone, Serialize)]
pub struct TranscriptResult {
    pub text: String,
    pub timestamp: String,
    pub source: String,
    pub sequence_id: u64,
    pub recording_start_time: f64,
    pub is_partial: bool,
    pub confidence: f32,
}

/// Streaming WhisperService - replaces multi-worker architecture
pub struct StreamingWhisperService<R: Runtime> {
    config: StreamingConfig,
    connection_manager: Arc<ConnectionManager>,
    vad_processor: Arc<RwLock<DualChannelVad>>,
    whisper_engine: Arc<RwLock<Option<WhisperEngine>>>,
    error_handler: Arc<ErrorHandler>,
    app_handle: AppHandle<R>,
    recording_start_time: Arc<RwLock<Option<Instant>>>,
    sequence_counter: Arc<std::sync::atomic::AtomicU64>,
}

impl<R: Runtime> StreamingWhisperService<R> {
    pub async fn new(app_handle: AppHandle<R>, config: StreamingConfig) -> Result<Self> {
        let vad_processor = DualChannelVad::new(16000)?;
        
        Ok(Self {
            config,
            connection_manager: Arc::new(ConnectionManager::new()),
            vad_processor: Arc::new(RwLock::new(vad_processor)),
            whisper_engine: Arc::new(RwLock::new(None)),
            error_handler: Arc::new(ErrorHandler::new()),
            app_handle,
            recording_start_time: Arc::new(RwLock::new(None)),
            sequence_counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        })
    }

    /// Initialize Whisper engine
    pub async fn initialize_whisper(&self, engine: WhisperEngine) -> Result<()> {
        let mut whisper_guard = self.whisper_engine.write().await;
        *whisper_guard = Some(engine);
        info!("Whisper engine initialized for streaming service");
        Ok(())
    }

    /// Start streaming transcription
    pub async fn start_streaming(&self) -> Result<ConnectionGuard> {
        let connection = self.connection_manager.acquire_connection().await;
        
        // Set recording start time
        let mut start_time = self.recording_start_time.write().await;
        *start_time = Some(Instant::now());
        
        // Reset VAD processor
        {
            let mut vad = self.vad_processor.write().await;
            vad.reset();
        }
        
        info!("Started streaming transcription with VAD-based segmentation");
        Ok(connection)
    }

    /// Process audio chunk with VAD segmentation
    pub async fn process_audio_chunk(&self, 
                                   mic_samples: &[f32], 
                                   speaker_samples: &[f32], 
                                   connection: &ConnectionGuard) -> Result<()> {
        if connection.is_cancelled() {
            return Ok(()); // Connection cancelled
        }

        // Process through VAD to get speech segments
        let speech_segments = {
            let mut vad = self.vad_processor.write().await;
            vad.process_dual_channel(mic_samples, speaker_samples).await?
        };

        // If VAD detected speech, transcribe it
        if !speech_segments.is_empty() {
            debug!("VAD detected {} speech samples, sending for transcription", speech_segments.len());
            self.transcribe_speech_segment(&speech_segments, connection).await?;
        } else {
            debug!("No speech detected by VAD, skipping transcription");
        }

        Ok(())
    }

    /// Transcribe speech segment (replaces broken accumulator logic)
    async fn transcribe_speech_segment(&self, speech_samples: &[f32], connection: &ConnectionGuard) -> Result<()> {
        if connection.is_cancelled() {
            return Ok(());
        }

        // Ensure minimum length for Whisper (increased for better context)
        let final_samples = if speech_samples.len() < 24000 { // 1.5 seconds minimum
            debug!("Speech segment too short ({} samples), padding to 1.5 seconds", speech_samples.len());
            let mut padded = speech_samples.to_vec();
            padded.resize(24000, 0.0);
            padded
        } else {
            speech_samples.to_vec()
        };

        // Get current timestamp
        let timestamp = {
            let start_time_guard = self.recording_start_time.read().await;
            if let Some(start_time) = *start_time_guard {
                start_time.elapsed().as_secs_f64()
            } else {
                0.0
            }
        };

        // Transcribe using Whisper
        let whisper_guard = self.whisper_engine.read().await;
        if let Some(whisper_engine) = whisper_guard.as_ref() {
            match whisper_engine.transcribe_audio(final_samples).await {
                Ok(text) => {
                    let cleaned_text = text.trim();
                    if !cleaned_text.is_empty() && cleaned_text != "you" { // Filter common false positives
                        let sequence_id = self.sequence_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        
                        let result = TranscriptResult {
                            text: cleaned_text.to_string(),
                            timestamp: format_timestamp(timestamp),
                            source: "Mixed Audio".to_string(),
                            sequence_id,
                            recording_start_time: timestamp,
                            is_partial: false,
                            confidence: 0.9, // VAD-filtered segments have high confidence
                        };

                        info!("Streaming transcription result: '{}' at {}", result.text, result.timestamp);

                        // Emit to frontend
                        if let Err(e) = self.app_handle.emit("transcript-update", &result) {
                            error!("Failed to emit transcript update: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Whisper transcription failed: {}", e);
                }
            }
        } else {
            error!("Whisper engine not initialized");
        }

        Ok(())
    }

    /// Stop streaming and clean up
    pub async fn stop_streaming(&self) -> Result<()> {
        info!("Stopping streaming transcription service");
        
        // Clear recording start time
        let mut start_time = self.recording_start_time.write().await;
        *start_time = None;

        // Emit completion event
        if let Err(e) = self.app_handle.emit("transcription-complete", ()) {
            error!("Failed to emit transcription-complete event: {}", e);
        }

        Ok(())
    }
}