use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use whisper_rs::{WhisperContext, WhisperState, FullParams, SamplingStrategy};
use serde::{Serialize, Deserialize};
use anyhow::{Result, anyhow};
use log::{debug, info, warn, error};
use std::time::{Duration, Instant};

use super::intelligent_chunking::{IntelligentChunker, ChunkedAudio, BoundaryType, AudioChunk};
use super::error::{AudioError, ErrorHandler, create_error_context};

/// Configuration for streaming whisper transcription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingWhisperConfig {
    /// Sample rate for audio processing
    pub sample_rate: usize,
    /// Maximum context window in samples (default: 30 seconds @ 16kHz = 480k samples)
    pub max_context_samples: usize,
    /// Overlap between chunks in samples (default: 1 second @ 16kHz = 16k samples)
    pub context_overlap_samples: usize,
    /// Maximum number of transcription retries
    pub max_retries: u32,
    /// Initial temperature for transcription
    pub base_temperature: f32,
    /// Temperature increment for retries
    pub temperature_increment: f32,
    /// Maximum temperature allowed
    pub max_temperature: f32,
    /// Language for transcription (None for auto-detect)
    pub language: Option<String>,
    /// Enable timestamp extraction
    pub enable_timestamps: bool,
    /// Confidence threshold for accepting transcripts
    pub confidence_threshold: f32,
    /// Maximum processing latency before timeout
    pub max_processing_time_ms: u64,
}

impl Default for StreamingWhisperConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            max_context_samples: 480000, // 30 seconds @ 16kHz
            context_overlap_samples: 16000, // 1 second @ 16kHz
            max_retries: 3,
            base_temperature: 0.0,
            temperature_increment: 0.2,
            max_temperature: 1.0,
            language: Some("en".to_string()),
            enable_timestamps: true,
            confidence_threshold: 0.3,
            max_processing_time_ms: 10000, // 10 seconds max processing
        }
    }
}

/// Result of streaming transcription
#[derive(Debug, Clone, Serialize)]
pub struct StreamingTranscriptionResult {
    pub text: String,
    pub confidence: f32,
    pub processing_time_ms: u64,
    pub retry_count: u32,
    pub temperature_used: f32,
    pub boundary_type: BoundaryType,
    pub has_context: bool,
    pub segment_timestamps: Vec<TranscriptionSegment>,
}

/// Individual transcription segment with timing
#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionSegment {
    pub text: String,
    pub start_ms: f64,
    pub end_ms: f64,
    pub confidence: f32,
}

/// Manages conversation context across chunks
#[derive(Debug)]
struct ContextManager {
    /// Rolling buffer of audio samples for context
    context_buffer: VecDeque<f32>,
    /// Text context from previous transcriptions
    text_context: VecDeque<String>,
    /// Maximum samples to keep in context
    max_context_samples: usize,
    /// Maximum text segments to keep
    max_text_segments: usize,
    /// Overlap samples for continuity
    overlap_samples: usize,
}

impl ContextManager {
    fn new(max_context_samples: usize, overlap_samples: usize) -> Self {
        Self {
            context_buffer: VecDeque::with_capacity(max_context_samples),
            text_context: VecDeque::with_capacity(10),
            max_context_samples,
            max_text_segments: 10,
            overlap_samples,
        }
    }

    /// Add new audio samples, maintaining context window
    fn add_audio_context(&mut self, samples: &[f32]) {
        // Add new samples
        for &sample in samples {
            self.context_buffer.push_back(sample);
        }

        // Trim to max size, keeping newest samples
        while self.context_buffer.len() > self.max_context_samples {
            self.context_buffer.pop_front();
        }

        debug!("Context buffer: {} samples (max: {})", 
               self.context_buffer.len(), self.max_context_samples);
    }

    /// Add text context from previous transcription
    fn add_text_context(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.text_context.push_back(text.to_string());
            
            // Trim to max segments
            while self.text_context.len() > self.max_text_segments {
                self.text_context.pop_front();
            }
        }
    }

    /// Get audio context for current transcription
    fn get_audio_context(&self, new_samples: &[f32]) -> Vec<f32> {
        let mut context_audio = Vec::new();
        
        // Add context from buffer (overlap region)
        if !self.context_buffer.is_empty() {
            let overlap_start = self.context_buffer.len().saturating_sub(self.overlap_samples);
            context_audio.extend_from_slice(&self.context_buffer.as_slices().0[overlap_start..]);
            if let Some(second_slice) = self.context_buffer.as_slices().1.get(overlap_start..) {
                context_audio.extend_from_slice(second_slice);
            }
        }

        // Add new samples
        context_audio.extend_from_slice(new_samples);

        debug!("Audio context: {} samples ({} context + {} new)", 
               context_audio.len(), context_audio.len() - new_samples.len(), new_samples.len());

        context_audio
    }

    /// Get text context for prompt engineering
    fn get_text_context(&self) -> String {
        if self.text_context.is_empty() {
            return String::new();
        }

        // Join last few segments with proper spacing
        self.text_context.iter().cloned().collect::<Vec<_>>().join(" ")
    }

    /// Reset all context
    fn reset(&mut self) {
        self.context_buffer.clear();
        self.text_context.clear();
    }
}

/// Temperature scheduler for retry logic
#[derive(Debug)]
struct TemperatureScheduler {
    base_temperature: f32,
    increment: f32,
    max_temperature: f32,
    current_retry: u32,
}

impl TemperatureScheduler {
    fn new(base: f32, increment: f32, max: f32) -> Self {
        Self {
            base_temperature: base,
            increment,
            max_temperature: max,
            current_retry: 0,
        }
    }

    /// Get temperature for current retry attempt
    fn get_temperature(&self) -> f32 {
        let temp = self.base_temperature + (self.current_retry as f32 * self.increment);
        temp.min(self.max_temperature)
    }

    /// Advance to next retry
    fn next_retry(&mut self) -> f32 {
        self.current_retry += 1;
        self.get_temperature()
    }

    /// Reset for new transcription
    fn reset(&mut self) {
        self.current_retry = 0;
    }
}

/// Streaming Whisper transcription service
pub struct StreamingWhisperService {
    /// Whisper context (shared across calls)
    whisper_context: Arc<RwLock<Option<WhisperContext>>>,
    /// Persistent whisper state for streaming
    whisper_state: Arc<RwLock<Option<WhisperState>>>,
    /// Intelligent chunker for boundary detection
    chunker: Arc<Mutex<IntelligentChunker>>,
    /// Context manager for conversation continuity
    context_manager: Arc<Mutex<ContextManager>>,
    /// Configuration
    config: StreamingWhisperConfig,
    /// Error handler
    error_handler: Arc<ErrorHandler>,
    /// Processing statistics
    stats: Arc<RwLock<StreamingStats>>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct StreamingStats {
    pub total_transcriptions: u64,
    pub total_processing_time_ms: u64,
    pub average_processing_time_ms: f64,
    pub retry_count: u64,
    pub error_count: u64,
    pub context_hits: u64,
    pub total_audio_samples: u64,
}

impl StreamingWhisperService {
    pub fn new(config: StreamingWhisperConfig) -> Result<Self> {
        let chunker_config = super::intelligent_chunking::ChunkingConfig {
            sample_rate: config.sample_rate as u32,
            min_chunk_duration_ms: 1000,
            max_chunk_duration_ms: 30000,
            target_chunk_duration_ms: 10000,
            overlap_duration_ms: (config.context_overlap_samples * 1000 / config.sample_rate) as u32,
            silence_threshold: 0.01,
            boundary_confidence_threshold: 0.8,
            force_chunk_on_silence_ms: 500,
            context_preservation_enabled: true,
        };

        let chunker = IntelligentChunker::new(chunker_config)?;
        
        let context_manager = ContextManager::new(
            config.max_context_samples,
            config.context_overlap_samples,
        );

        Ok(Self {
            whisper_context: Arc::new(RwLock::new(None)),
            whisper_state: Arc::new(RwLock::new(None)),
            chunker: Arc::new(Mutex::new(chunker)),
            context_manager: Arc::new(Mutex::new(context_manager)),
            config,
            error_handler: Arc::new(ErrorHandler::new()),
            stats: Arc::new(RwLock::new(StreamingStats::default())),
        })
    }

    /// Initialize with a whisper context (call this after loading a model)
    pub async fn initialize(&self, whisper_context: WhisperContext) -> Result<()> {
        // Create persistent state
        let state = whisper_context.create_state()
            .map_err(|e| anyhow!("Failed to create whisper state: {}", e))?;

        *self.whisper_context.write().await = Some(whisper_context);
        *self.whisper_state.write().await = Some(state);

        info!("StreamingWhisperService initialized with persistent state");
        Ok(())
    }

    /// Process streaming audio with intelligent chunking
    pub async fn process_streaming_audio(&self, audio_samples: &[f32]) -> Result<Vec<StreamingTranscriptionResult>> {
        let start_time = Instant::now();
        let mut results = Vec::new();

        // Process audio through intelligent chunker
        let chunked_audio = {
            let mut chunker = self.chunker.lock().await;
            chunker.process_stream(audio_samples).await
                .map_err(|e| AudioError::chunk_processing_failed(audio_samples.len(), &e.to_string()))?
        };

        debug!("Intelligent chunker produced {} chunks", chunked_audio.ready_chunks.len());

        // Process each ready chunk
        for chunk in chunked_audio.ready_chunks {
            match self.transcribe_chunk(&chunk).await {
                Ok(result) => {
                    // Update context with successful transcription
                    {
                        let mut context = self.context_manager.lock().await;
                        context.add_audio_context(&chunk.samples);
                        context.add_text_context(&result.text);
                    }

                    results.push(result);
                }
                Err(e) => {
                    let error = AudioError::transcription_failed(chunk.samples.len(), &e.to_string());
                    let context = create_error_context("streaming_whisper", "transcribe_chunk", None);
                    let _action = self.error_handler.handle_error(error, context).await;

                    warn!("Failed to transcribe chunk: {}", e);
                    // Continue with other chunks instead of failing completely
                }
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_transcriptions += results.len() as u64;
            stats.total_processing_time_ms += start_time.elapsed().as_millis() as u64;
            stats.average_processing_time_ms = stats.total_processing_time_ms as f64 / stats.total_transcriptions.max(1) as f64;
            stats.total_audio_samples += audio_samples.len() as u64;
        }

        info!("Processed {} samples into {} transcription results in {:?}", 
              audio_samples.len(), results.len(), start_time.elapsed());

        Ok(results)
    }

    /// Transcribe a single chunk with context and retry logic
    async fn transcribe_chunk(&self, chunk: &AudioChunk) -> Result<StreamingTranscriptionResult> {
        let chunk_start_time = Instant::now();

        // Get audio context for this chunk
        let (audio_with_context, text_context, has_context) = {
            let context_manager = self.context_manager.lock().await;
            let audio_context = context_manager.get_audio_context(&chunk.samples);
            let text_context = context_manager.get_text_context();
            let has_context = !text_context.is_empty();
            (audio_context, text_context, has_context)
        };

        debug!("Transcribing chunk: {} samples, boundary: {:?}, has_context: {}", 
               chunk.samples.len(), chunk.metadata.boundary_type, has_context);

        // Setup temperature scheduler for retries
        let mut temp_scheduler = TemperatureScheduler::new(
            self.config.base_temperature,
            self.config.temperature_increment,
            self.config.max_temperature,
        );

        let mut last_error = None;

        // Retry logic with temperature scheduling
        for retry in 0..=self.config.max_retries {
            let temperature = temp_scheduler.get_temperature();
            
            match self.perform_transcription(&audio_with_context, temperature, &text_context).await {
                Ok(result) => {
                    let processing_time = chunk_start_time.elapsed().as_millis() as u64;

                    // Update context hit statistics
                    if has_context {
                        let mut stats = self.stats.write().await;
                        stats.context_hits += 1;
                    }

                    debug!("Transcription successful on attempt {} with temperature {:.2}: '{}'", 
                           retry + 1, temperature, result.text);

                    return Ok(StreamingTranscriptionResult {
                        text: result.text,
                        confidence: result.confidence,
                        processing_time_ms: processing_time,
                        retry_count: retry,
                        temperature_used: temperature,
                        boundary_type: chunk.metadata.boundary_type.clone(),
                        has_context,
                        segment_timestamps: result.segments,
                    });
                }
                Err(e) => {
                    warn!("Transcription attempt {} failed with temperature {:.2}: {}", retry + 1, temperature, e);
                    last_error = Some(e);
                    
                    if retry < self.config.max_retries {
                        temp_scheduler.next_retry();
                        // Brief delay before retry
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }

            // Check for timeout
            if chunk_start_time.elapsed().as_millis() > self.config.max_processing_time_ms as u128 {
                let error = AudioError::processing_timeout(
                    chunk.samples.len(),
                    self.config.max_processing_time_ms,
                );
                let context = create_error_context("streaming_whisper", "transcribe_chunk_timeout", None);
                let _action = self.error_handler.handle_error(error, context).await;

                return Err(anyhow!("Transcription timeout after {}ms", self.config.max_processing_time_ms));
            }
        }

        // Update error statistics
        {
            let mut stats = self.stats.write().await;
            stats.error_count += 1;
            stats.retry_count += self.config.max_retries as u64;
        }

        Err(last_error.unwrap_or_else(|| anyhow!("Transcription failed after {} retries", self.config.max_retries)))
    }

    /// Perform actual whisper transcription
    async fn perform_transcription(&self, audio_samples: &[f32], temperature: f32, text_context: &str) -> Result<TranscriptionAttemptResult> {
        let ctx_lock = self.whisper_context.read().await;
        let ctx = ctx_lock.as_ref()
            .ok_or_else(|| anyhow!("No whisper context available"))?;

        let mut state_lock = self.whisper_state.write().await;
        let state = state_lock.as_mut()
            .ok_or_else(|| anyhow!("No whisper state available"))?;

        // Create transcription parameters with temperature
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        
        // Configure parameters
        if let Some(ref lang) = self.config.language {
            params.set_language(Some(lang));
        }
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(self.config.enable_timestamps);
        params.set_temperature(temperature);

        // Use text context as initial prompt if available
        if !text_context.is_empty() {
            params.set_initial_prompt(text_context);
            debug!("Using text context as prompt: '{}'", text_context);
        }

        // Run transcription
        state.full(params, audio_samples)
            .map_err(|e| anyhow!("Whisper transcription failed: {}", e))?;

        // Extract results
        let num_segments = state.full_n_segments()
            .map_err(|e| anyhow!("Failed to get segment count: {}", e))?;

        let mut text_result = String::new();
        let mut segments = Vec::new();
        let mut total_confidence = 0.0;

        for i in 0..num_segments {
            let segment_text = state.full_get_segment_text(i)
                .map_err(|e| anyhow!("Failed to get segment text: {}", e))?;

            if !segment_text.trim().is_empty() {
                text_result.push_str(&segment_text);
                if i < num_segments - 1 {
                    text_result.push(' ');
                }

                // Extract timing if enabled
                if self.config.enable_timestamps {
                    let start_time = state.full_get_segment_t0(i).unwrap_or(0) as f64 * 10.0; // Convert to ms
                    let end_time = state.full_get_segment_t1(i).unwrap_or(0) as f64 * 10.0; // Convert to ms
                    
                    // Rough confidence estimation (would need more sophisticated approach in real implementation)
                    let segment_confidence = 0.8; // Placeholder - whisper doesn't directly provide this
                    total_confidence += segment_confidence;

                    segments.push(TranscriptionSegment {
                        text: segment_text.trim().to_string(),
                        start_ms: start_time,
                        end_ms: end_time,
                        confidence: segment_confidence,
                    });
                }
            }
        }

        let average_confidence = if num_segments > 0 { 
            total_confidence / num_segments as f32 
        } else { 
            0.0 
        };

        // Check confidence threshold
        if average_confidence < self.config.confidence_threshold {
            return Err(anyhow!("Transcription confidence {:.2} below threshold {:.2}", 
                              average_confidence, self.config.confidence_threshold));
        }

        Ok(TranscriptionAttemptResult {
            text: text_result.trim().to_string(),
            confidence: average_confidence,
            segments,
        })
    }

    /// Reset all streaming context
    pub async fn reset_context(&self) {
        let mut context = self.context_manager.lock().await;
        context.reset();
        
        let mut chunker = self.chunker.lock().await;
        chunker.reset();

        info!("StreamingWhisperService context reset");
    }

    /// Get processing statistics
    pub async fn get_statistics(&self) -> StreamingStats {
        (*self.stats.read().await).clone()
    }

    /// Check if service is ready for transcription
    pub async fn is_ready(&self) -> bool {
        let ctx_lock = self.whisper_context.read().await;
        let state_lock = self.whisper_state.read().await;
        ctx_lock.is_some() && state_lock.is_some()
    }
}

/// Internal result structure for transcription attempts
#[derive(Debug)]
struct TranscriptionAttemptResult {
    text: String,
    confidence: f32,
    segments: Vec<TranscriptionSegment>,
}

impl BoundaryType {
    fn to_string(&self) -> String {
        match self {
            BoundaryType::SpeechEnd => "SpeechEnd".to_string(),
            BoundaryType::Silence => "Silence".to_string(),
            BoundaryType::MaxDuration => "MaxDuration".to_string(),
            BoundaryType::EnergyDrop => "EnergyDrop".to_string(),
            BoundaryType::PitchChange => "PitchChange".to_string(),
            BoundaryType::SentenceBoundary => "SentenceBoundary".to_string(),
            BoundaryType::PauseBoundary => "PauseBoundary".to_string(),
            BoundaryType::TimeoutBoundary => "TimeoutBoundary".to_string(),
            BoundaryType::MaxDurationBoundary => "MaxDurationBoundary".to_string(),
            BoundaryType::SilenceBoundary => "SilenceBoundary".to_string(),
            BoundaryType::ManualBoundary => "ManualBoundary".to_string(),
        }
    }
}