use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};
use anyhow::{Result, anyhow};
use log::{debug, info, warn, error};

use super::streaming_vad::{StreamingVadProcessor, BoundaryInfo, StreamingVadConfig};
use super::error::{AudioError, ErrorHandler, create_error_context};

/// Configuration for intelligent chunking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingConfig {
    pub min_chunk_duration_ms: u32,
    pub max_chunk_duration_ms: u32,
    pub target_chunk_duration_ms: u32,
    pub sample_rate: u32,
    pub overlap_duration_ms: u32,
    pub silence_threshold: f32,
    pub boundary_confidence_threshold: f32,
    pub force_chunk_on_silence_ms: u32,
    pub context_preservation_enabled: bool,
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self {
            min_chunk_duration_ms: 3000,  // 3 seconds minimum for better context
            max_chunk_duration_ms: 30000, // 30 seconds maximum
            target_chunk_duration_ms: 15000, // 15 seconds target (optimal for Whisper context)
            sample_rate: 16000,
            overlap_duration_ms: 500, // 500ms overlap for better continuity
            silence_threshold: 0.001, // Less aggressive silence detection
            boundary_confidence_threshold: 0.8, // Higher confidence required
            force_chunk_on_silence_ms: 8000, // Force chunk after 8s of silence (increased)
            context_preservation_enabled: true,
        }
    }
}

/// Result of chunking operation with multiple chunks ready for processing
#[derive(Debug, Clone, Serialize)]
pub struct ChunkedAudio {
    pub ready_chunks: Vec<AudioChunk>,
    pub partial_chunk: Option<Vec<f32>>,
    pub statistics: ChunkingStatistics,
}

/// Metadata associated with an audio chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub chunk_id: u64,
    pub timestamp: f64,
    pub duration_ms: u32,
    pub sample_count: usize,
    pub has_speech_boundary: bool,
    pub confidence: f32,
    pub energy_level: f32,
    pub noise_floor: f32,
    pub context_frames: usize,
    pub is_silence_forced: bool,
    pub boundary_type: BoundaryType,
}

/// Types of boundaries that can trigger chunking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BoundaryType {
    SpeechEnd,
    Silence,
    MaxDuration,
    EnergyDrop,
    PitchChange,
    SentenceBoundary,
    PauseBoundary,
    TimeoutBoundary,
    MaxDurationBoundary,
    SilenceBoundary,
    ManualBoundary,
}

/// Audio chunk with context and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub metadata: ChunkMetadata,
    pub start_time_ms: u64,  // milliseconds since recording start
    pub recording_start_time_ms: u64,  // milliseconds since epoch
}

/// Context buffer for preserving audio continuity
pub struct ContextBuffer {
    samples: VecDeque<f32>,
    max_context_samples: usize,
    overlap_samples: usize,
}

impl ContextBuffer {
    fn new(max_duration_ms: u32, overlap_duration_ms: u32, sample_rate: u32) -> Self {
        let max_context_samples = (max_duration_ms as f32 / 1000.0 * sample_rate as f32) as usize;
        let overlap_samples = (overlap_duration_ms as f32 / 1000.0 * sample_rate as f32) as usize;
        
        Self {
            samples: VecDeque::new(),
            max_context_samples,
            overlap_samples,
        }
    }

    fn add_samples(&mut self, new_samples: &[f32]) {
        self.samples.extend(new_samples);
        
        // Maintain buffer size
        while self.samples.len() > self.max_context_samples {
            self.samples.pop_front();
        }
    }

    fn get_context_for_new_chunk(&self) -> Vec<f32> {
        let overlap_count = self.overlap_samples.min(self.samples.len());
        self.samples
            .range(self.samples.len().saturating_sub(overlap_count)..)
            .copied()
            .collect()
    }

    fn append_with_overlap(&mut self, new_samples: Vec<f32>) -> Vec<f32> {
        let context = self.get_context_for_new_chunk();
        self.add_samples(&new_samples);
        
        let mut result = Vec::with_capacity(context.len() + new_samples.len());
        result.extend(context);
        result.extend(new_samples);
        result
    }

    fn len(&self) -> usize {
        self.samples.len()
    }

    fn duration_ms(&self, sample_rate: u32) -> u32 {
        (self.samples.len() as f32 / sample_rate as f32 * 1000.0) as u32
    }

    fn clear(&mut self) {
        self.samples.clear();
    }
}

/// Intelligent chunker that creates chunks based on speech boundaries
pub struct IntelligentChunker {
    config: ChunkingConfig,
    vad_processor: StreamingVadProcessor,
    context_buffer: ContextBuffer,
    current_chunk: Vec<f32>,
    chunk_start_time: Option<Instant>,
    last_boundary_time: Option<Instant>,
    chunk_id_counter: AtomicU64,
    silence_start_time: Option<Instant>,
    total_processed_samples: u64,
    error_handler: Arc<ErrorHandler>,
}

impl IntelligentChunker {
    pub fn new(config: ChunkingConfig) -> Result<Self> {
        let vad_config = StreamingVadConfig {
            sample_rate: config.sample_rate as usize,
            frame_duration_ms: 30,
            redemption_time_ms: 200,
            pre_speech_pad_ms: 100,
            post_speech_pad_ms: 150,
            min_speech_duration_ms: 300,
            adaptive_threshold: true,
            energy_threshold: config.silence_threshold,
            zero_crossing_threshold: 0.1,
            pitch_detection_enabled: true,
        };

        let context_buffer = ContextBuffer::new(
            config.max_chunk_duration_ms,
            config.overlap_duration_ms,
            config.sample_rate,
        );

        Ok(Self {
            vad_processor: StreamingVadProcessor::new(vad_config)?,
            context_buffer,
            current_chunk: Vec::new(),
            chunk_start_time: None,
            last_boundary_time: None,
            chunk_id_counter: AtomicU64::new(0),
            silence_start_time: None,
            total_processed_samples: 0,
            config,
            error_handler: Arc::new(ErrorHandler::new()),
        })
    }

    /// Process audio samples and create chunks when appropriate
    pub async fn process_audio(&mut self, samples: &[f32], recording_start_time: Instant) -> Result<Option<AudioChunk>> {
        if samples.is_empty() {
            return Ok(None);
        }

        self.total_processed_samples += samples.len() as u64;

        // Process through VAD to get boundary information
        let vad_result = match self.vad_processor.process_stream(samples).await {
            Ok(result) => result,
            Err(e) => {
                let error = AudioError::vad_processing_failed(samples.len(), &e.to_string());
                let context = create_error_context("intelligent_chunker", "vad_processing", None);
                let _action = self.error_handler.handle_error(error, context).await;
                
                warn!("VAD processing failed, using fallback chunking: {}", e);
                return self.create_fallback_chunk(samples, recording_start_time);
            }
        };

        // Add samples to current chunk
        self.current_chunk.extend_from_slice(samples);
        
        // Initialize chunk start time if this is a new chunk
        if self.chunk_start_time.is_none() {
            self.chunk_start_time = Some(Instant::now());
        }

        // Check for silence tracking
        self.update_silence_tracking(&vad_result);

        // Determine if we should create a chunk
        let chunk_decision = self.should_create_chunk(&vad_result).await;

        match chunk_decision {
            ChunkDecision::CreateChunk(boundary_type) => {
                self.create_chunk(boundary_type, &vad_result, recording_start_time)
            }
            ChunkDecision::Continue => Ok(None),
        }
    }

    /// Update silence tracking for timeout-based chunking
    fn update_silence_tracking(&mut self, vad_result: &super::streaming_vad::StreamingResult) {
        let is_silent = vad_result.energy_level < self.config.silence_threshold && 
                       !vad_result.is_speaking;

        if is_silent && self.silence_start_time.is_none() {
            self.silence_start_time = Some(Instant::now());
        } else if !is_silent {
            self.silence_start_time = None;
        }
    }

    /// Determine if we should create a chunk based on current conditions
    async fn should_create_chunk(&self, vad_result: &super::streaming_vad::StreamingResult) -> ChunkDecision {
        let current_duration = self.get_current_chunk_duration_ms();

        // Force chunk on maximum duration
        if current_duration >= self.config.max_chunk_duration_ms {
            debug!("Force chunk: maximum duration reached ({}ms)", current_duration);
            return ChunkDecision::CreateChunk(BoundaryType::MaxDurationBoundary);
        }

        // Only consider other boundaries if we've met minimum duration
        if current_duration < self.config.min_chunk_duration_ms {
            return ChunkDecision::Continue;
        }

        // Check for speech boundaries with sufficient confidence
        if vad_result.boundary_info.is_complete_utterance && 
           vad_result.confidence >= self.config.boundary_confidence_threshold {
            debug!("Create chunk: speech boundary detected (confidence: {:.2})", vad_result.confidence);
            return ChunkDecision::CreateChunk(BoundaryType::SentenceBoundary);
        }

        // Check for silence-based chunking
        if let Some(silence_start) = self.silence_start_time {
            let silence_duration = silence_start.elapsed().as_millis() as u32;
            if silence_duration >= self.config.force_chunk_on_silence_ms {
                debug!("Create chunk: prolonged silence ({}ms)", silence_duration);
                return ChunkDecision::CreateChunk(BoundaryType::SilenceBoundary);
            }
        }

        // Check for natural pauses
        if !vad_result.boundary_info.sentence_boundaries.is_empty() && 
           current_duration >= self.config.target_chunk_duration_ms * 2 / 3 {
            debug!("Create chunk: natural pause detected");
            return ChunkDecision::CreateChunk(BoundaryType::PauseBoundary);
        }

        // Check for target duration with good stopping point
        if current_duration >= self.config.target_chunk_duration_ms && 
           (vad_result.confidence > 0.4 || !vad_result.is_speaking) {
            debug!("Create chunk: target duration with good stopping point");
            return ChunkDecision::CreateChunk(BoundaryType::TimeoutBoundary);
        }

        ChunkDecision::Continue
    }

    /// Create a chunk with proper metadata
    fn create_chunk(
        &mut self, 
        boundary_type: BoundaryType, 
        vad_result: &super::streaming_vad::StreamingResult,
        recording_start_time: Instant,
    ) -> Result<Option<AudioChunk>> {
        if self.current_chunk.is_empty() {
            return Ok(None);
        }

        let chunk_id = self.chunk_id_counter.fetch_add(1, Ordering::SeqCst);
        let chunk_start_time = self.chunk_start_time.unwrap_or_else(Instant::now);
        let duration_ms = chunk_start_time.elapsed().as_millis() as u32;

        // Calculate chunk timestamp relative to recording start
        let timestamp = recording_start_time.elapsed().as_secs_f64();

        // Prepare samples with context if enabled
        let final_samples = if self.config.context_preservation_enabled {
            self.context_buffer.append_with_overlap(self.current_chunk.clone())
        } else {
            self.context_buffer.add_samples(&self.current_chunk);
            self.current_chunk.clone()
        };

        // Create metadata
        let metadata = ChunkMetadata {
            chunk_id,
            timestamp,
            duration_ms,
            sample_count: final_samples.len(),
            has_speech_boundary: vad_result.boundary_info.is_complete_utterance,
            confidence: vad_result.confidence,
            energy_level: vad_result.energy_level,
            noise_floor: vad_result.noise_floor,
            context_frames: self.context_buffer.len(),
            is_silence_forced: matches!(boundary_type, BoundaryType::SilenceBoundary),
            boundary_type: boundary_type.clone(),
        };

        let chunk = AudioChunk {
            samples: final_samples,
            metadata,
            start_time_ms: chunk_start_time.elapsed().as_millis() as u64,
            recording_start_time_ms: recording_start_time.elapsed().as_millis() as u64,
        };

        info!("📦 Created intelligent chunk #{} ({:.2}s, {} samples, {:?})", 
              chunk_id, duration_ms as f32 / 1000.0, chunk.samples.len(), boundary_type);

        // Reset for next chunk
        self.reset_chunk_state();

        Ok(Some(chunk))
    }

    /// Create fallback chunk when VAD fails
    fn create_fallback_chunk(&mut self, samples: &[f32], recording_start_time: Instant) -> Result<Option<AudioChunk>> {
        self.current_chunk.extend_from_slice(samples);
        
        if self.chunk_start_time.is_none() {
            self.chunk_start_time = Some(Instant::now());
        }

        let current_duration = self.get_current_chunk_duration_ms();
        
        // Use simple duration-based chunking as fallback
        if current_duration >= self.config.target_chunk_duration_ms {
            let chunk_id = self.chunk_id_counter.fetch_add(1, Ordering::SeqCst);
            let chunk_start_time = self.chunk_start_time.unwrap_or_else(Instant::now);
            let timestamp = recording_start_time.elapsed().as_secs_f64();
            
            let samples = self.current_chunk.clone();
            let metadata = ChunkMetadata {
                chunk_id,
                timestamp,
                duration_ms: current_duration,
                sample_count: samples.len(),
                has_speech_boundary: false,
                confidence: 0.3, // Low confidence for fallback
                energy_level: samples.iter().map(|&x| x * x).sum::<f32>() / samples.len() as f32,
                noise_floor: 0.001,
                context_frames: 0,
                is_silence_forced: false,
                boundary_type: BoundaryType::TimeoutBoundary,
            };

            let chunk = AudioChunk {
                samples,
                metadata,
                start_time_ms: chunk_start_time.elapsed().as_millis() as u64,
                recording_start_time_ms: recording_start_time.elapsed().as_millis() as u64,
            };

            warn!("📦 Created fallback chunk #{} ({:.2}s, {} samples)", 
                  chunk_id, current_duration as f32 / 1000.0, chunk.samples.len());

            self.reset_chunk_state();
            return Ok(Some(chunk));
        }

        Ok(None)
    }

    /// Reset chunk state for next chunk
    fn reset_chunk_state(&mut self) {
        self.current_chunk.clear();
        self.chunk_start_time = None;
        self.last_boundary_time = Some(Instant::now());
        self.silence_start_time = None;
    }

    /// Get current chunk duration in milliseconds
    fn get_current_chunk_duration_ms(&self) -> u32 {
        match self.chunk_start_time {
            Some(start_time) => start_time.elapsed().as_millis() as u32,
            None => 0,
        }
    }

    /// Force create a chunk (for manual boundary creation)
    pub async fn force_chunk(&mut self, recording_start_time: Instant) -> Result<Option<AudioChunk>> {
        if self.current_chunk.is_empty() {
            return Ok(None);
        }

        // Create a synthetic VAD result for forced chunk
        let vad_result = super::streaming_vad::StreamingResult {
            speech_segments: vec![self.current_chunk.clone()],
            is_speaking: false,
            confidence: 0.5,
            boundary_info: super::streaming_vad::BoundaryInfo {
                sentence_boundaries: vec![],
                word_boundaries: vec![],
                is_complete_utterance: false,
                confidence: 0.5,
                speech_probability: 0.5,
            },
            noise_floor: 0.001,
            energy_level: 0.01,
        };

        self.create_chunk(BoundaryType::ManualBoundary, &vad_result, recording_start_time)
    }

    /// Get chunking statistics
    pub fn get_statistics(&self) -> ChunkingStatistics {
        ChunkingStatistics {
            total_chunks_created: self.chunk_id_counter.load(Ordering::Relaxed),
            current_chunk_duration_ms: self.get_current_chunk_duration_ms(),
            current_chunk_samples: self.current_chunk.len(),
            total_processed_samples: self.total_processed_samples,
            context_buffer_size: self.context_buffer.len(),
            vad_stats: self.vad_processor.get_statistics(),
        }
    }

    /// Update configuration
    pub fn update_config(&mut self, config: ChunkingConfig) -> Result<()> {
        self.config = config.clone();
        
        // Update VAD configuration
        let vad_config = StreamingVadConfig {
            sample_rate: config.sample_rate as usize,
            frame_duration_ms: 30,
            redemption_time_ms: 200,
            pre_speech_pad_ms: 100,
            post_speech_pad_ms: 150,
            min_speech_duration_ms: 300,
            adaptive_threshold: true,
            energy_threshold: config.silence_threshold,
            zero_crossing_threshold: 0.1,
            pitch_detection_enabled: true,
        };
        
        self.vad_processor.update_config(vad_config);
        
        // Reset context buffer with new settings
        self.context_buffer = ContextBuffer::new(
            config.max_chunk_duration_ms,
            config.overlap_duration_ms,
            config.sample_rate,
        );
        
        info!("Intelligent chunker configuration updated");
        Ok(())
    }

    /// Process streaming audio and return batched chunks
    pub async fn process_stream(&mut self, samples: &[f32]) -> Result<ChunkedAudio> {
        let recording_start = Instant::now();
        let mut ready_chunks = Vec::new();
        
        // Process audio in smaller chunks to maintain responsiveness
        let chunk_size = self.config.sample_rate as usize / 10; // 100ms chunks
        let mut partial_chunk = None;
        
        for chunk_start in (0..samples.len()).step_by(chunk_size) {
            let chunk_end = (chunk_start + chunk_size).min(samples.len());
            let audio_chunk = &samples[chunk_start..chunk_end];
            
            if let Some(processed_chunk) = self.process_audio(audio_chunk, recording_start).await? {
                ready_chunks.push(processed_chunk);
            }
        }
        
        // If we have remaining data in current chunk, save as partial
        if !self.current_chunk.is_empty() {
            partial_chunk = Some(self.current_chunk.clone());
        }
        
        Ok(ChunkedAudio {
            ready_chunks,
            partial_chunk,
            statistics: self.get_statistics(),
        })
    }

    /// Reset chunker state
    pub fn reset(&mut self) {
        self.current_chunk.clear();
        self.chunk_start_time = None;
        self.last_boundary_time = None;
        self.silence_start_time = None;
        self.context_buffer.clear();
        self.vad_processor.reset();
        self.total_processed_samples = 0;
        info!("Intelligent chunker reset");
    }
}

/// Decision about whether to create a chunk
#[derive(Debug)]
enum ChunkDecision {
    CreateChunk(BoundaryType),
    Continue,
}

/// Statistics for monitoring chunker performance
#[derive(Debug, Clone, Serialize)]
pub struct ChunkingStatistics {
    pub total_chunks_created: u64,
    pub current_chunk_duration_ms: u32,
    pub current_chunk_samples: usize,
    pub total_processed_samples: u64,
    pub context_buffer_size: usize,
    pub vad_stats: super::streaming_vad::VadStatistics,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_intelligent_chunker_basic() {
        let config = ChunkingConfig::default();
        let mut chunker = IntelligentChunker::new(config).unwrap();
        
        let recording_start = Instant::now();
        
        // Test with speech-like signal
        let mut samples = Vec::new();
        for i in 0..16000 { // 1 second at 16kHz
            samples.push((i as f32 * 0.1).sin() * 0.1);
        }
        
        let result = chunker.process_audio(&samples, recording_start).await.unwrap();
        // Shouldn't create chunk yet (below minimum duration)
        assert!(result.is_none());
        
        // Add more samples to reach minimum duration
        for _ in 0..5 {
            let result = chunker.process_audio(&samples, recording_start).await.unwrap();
            if result.is_some() {
                let chunk = result.unwrap();
                assert!(chunk.metadata.duration_ms >= 1000); // At least 1 second
                break;
            }
        }
    }

    #[tokio::test]
    async fn test_force_chunk() {
        let config = ChunkingConfig::default();
        let mut chunker = IntelligentChunker::new(config).unwrap();
        
        let recording_start = Instant::now();
        let samples: Vec<f32> = (0..8000).map(|i| (i as f32 * 0.1).sin() * 0.1).collect();
        
        // Add some samples
        chunker.process_audio(&samples, recording_start).await.unwrap();
        
        // Force chunk creation
        let result = chunker.force_chunk(recording_start).await.unwrap();
        assert!(result.is_some());
        
        let chunk = result.unwrap();
        assert_eq!(chunk.metadata.boundary_type, BoundaryType::ManualBoundary);
        assert!(chunk.samples.len() > 0);
    }

    #[tokio::test]
    async fn test_silence_forced_chunking() {
        let mut config = ChunkingConfig::default();
        config.force_chunk_on_silence_ms = 100; // Very short for testing
        config.min_chunk_duration_ms = 50;      // Very short for testing
        
        let mut chunker = IntelligentChunker::new(config).unwrap();
        let recording_start = Instant::now();
        
        // Add some speech samples first
        let speech: Vec<f32> = (0..1600).map(|i| (i as f32 * 0.1).sin() * 0.1).collect(); // 0.1s
        chunker.process_audio(&speech, recording_start).await.unwrap();
        
        // Add silence
        let silence = vec![0.0; 1600]; // 0.1s of silence
        
        // Process silence multiple times to trigger timeout
        for _ in 0..3 {
            if let Some(chunk) = chunker.process_audio(&silence, recording_start).await.unwrap() {
                assert!(matches!(chunk.metadata.boundary_type, 
                    BoundaryType::SilenceBoundary | BoundaryType::TimeoutBoundary));
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
    }

    #[test]
    fn test_context_buffer() {
        let mut buffer = ContextBuffer::new(1000, 200, 16000); // 1s max, 200ms overlap
        
        let samples1: Vec<f32> = (0..8000).map(|i| i as f32).collect(); // 0.5s
        let samples2: Vec<f32> = (8000..16000).map(|i| i as f32).collect(); // 0.5s
        
        buffer.add_samples(&samples1);
        assert_eq!(buffer.len(), 8000);
        
        let context = buffer.get_context_for_new_chunk();
        assert!(context.len() <= 3200); // 200ms overlap at 16kHz
        
        let with_overlap = buffer.append_with_overlap(samples2);
        assert!(with_overlap.len() > 8000); // Should include context
    }

    #[test]
    fn test_chunking_statistics() {
        let config = ChunkingConfig::default();
        let chunker = IntelligentChunker::new(config).unwrap();
        
        let stats = chunker.get_statistics();
        assert_eq!(stats.total_chunks_created, 0);
        assert_eq!(stats.current_chunk_samples, 0);
        assert_eq!(stats.total_processed_samples, 0);
    }
}