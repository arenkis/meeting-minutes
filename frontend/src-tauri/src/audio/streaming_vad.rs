use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use serde::{Serialize, Deserialize};
use anyhow::{Result, anyhow};
use log::{debug, info, warn, error};

use super::error::{AudioError, ErrorHandler, create_error_context};
use super::buffer::AdaptiveBuffer;

/// Configuration for streaming VAD processor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingVadConfig {
    pub sample_rate: usize,
    pub frame_duration_ms: u32,
    pub redemption_time_ms: u32,
    pub pre_speech_pad_ms: u32,
    pub post_speech_pad_ms: u32,
    pub min_speech_duration_ms: u32,
    pub adaptive_threshold: bool,
    pub energy_threshold: f32,
    pub zero_crossing_threshold: f32,
    pub pitch_detection_enabled: bool,
}

impl Default for StreamingVadConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            frame_duration_ms: 30, // Optimal frame size from  analysis
            redemption_time_ms: 800,  // Increased to prevent premature speech ending
            pre_speech_pad_ms: 300,   // More pre-context for better transcription
            post_speech_pad_ms: 500,  // More post-context to capture speech tails
            min_speech_duration_ms: 500, // Higher minimum to filter out noise
            adaptive_threshold: true,
            energy_threshold: 0.002, // Slightly less aggressive
            zero_crossing_threshold: 0.15, // More tolerant of speech variations
            pitch_detection_enabled: true,
        }
    }
}

/// Speech boundary information
#[derive(Debug, Clone, Serialize)]
pub struct BoundaryInfo {
    pub sentence_boundaries: Vec<usize>,
    pub word_boundaries: Vec<usize>,
    pub is_complete_utterance: bool,
    pub confidence: f32,
    pub speech_probability: f32,
}

/// Streaming result from VAD processing
#[derive(Debug, Clone)]
pub struct StreamingResult {
    pub speech_segments: Vec<Vec<f32>>,
    pub is_speaking: bool,
    pub confidence: f32,
    pub boundary_info: BoundaryInfo,
    pub noise_floor: f32,
    pub energy_level: f32,
}

/// Adaptive noise floor estimator
struct AdaptiveNoiseEstimator {
    noise_samples: VecDeque<f32>,
    current_noise_floor: f32,
    adaptation_rate: f32,
    max_samples: usize,
}

impl AdaptiveNoiseEstimator {
    fn new() -> Self {
        Self {
            noise_samples: VecDeque::new(),
            current_noise_floor: 0.001, // Initial estimate
            adaptation_rate: 0.01,
            max_samples: 1000, // ~30 seconds of 30ms frames
        }
    }

    fn update(&mut self, samples: &[f32]) {
        let rms_energy = calculate_rms_energy(samples);
        
        // Add to noise samples if energy is low (likely noise)
        if rms_energy < self.current_noise_floor * 2.0 {
            self.noise_samples.push_back(rms_energy);
            
            if self.noise_samples.len() > self.max_samples {
                self.noise_samples.pop_front();
            }
            
            // Update noise floor using exponential moving average
            if !self.noise_samples.is_empty() {
                let avg_noise: f32 = self.noise_samples.iter().sum::<f32>() / self.noise_samples.len() as f32;
                self.current_noise_floor = (1.0 - self.adaptation_rate) * self.current_noise_floor + 
                                         self.adaptation_rate * avg_noise;
            }
        }
    }

    fn noise_floor(&self) -> f32 {
        self.current_noise_floor
    }
    
    fn adaptive_threshold(&self) -> f32 {
        (self.current_noise_floor * 3.0).max(0.002).min(0.01)
    }
}

/// Energy tracker for speech activity detection
struct EnergyTracker {
    recent_energy: VecDeque<f32>,
    window_size: usize,
    high_energy_count: u32,
    low_energy_count: u32,
}

impl EnergyTracker {
    fn new(window_size: usize) -> Self {
        Self {
            recent_energy: VecDeque::new(),
            window_size,
            high_energy_count: 0,
            low_energy_count: 0,
        }
    }

    fn calculate(&mut self, samples: &[f32]) -> f32 {
        let energy = calculate_rms_energy(samples);
        
        self.recent_energy.push_back(energy);
        if self.recent_energy.len() > self.window_size {
            let old_energy = self.recent_energy.pop_front().unwrap();
            if old_energy > 0.005 {
                self.high_energy_count = self.high_energy_count.saturating_sub(1);
            } else {
                self.low_energy_count = self.low_energy_count.saturating_sub(1);
            }
        }
        
        // Track high/low energy frames
        if energy > 0.005 {
            self.high_energy_count += 1;
        } else {
            self.low_energy_count += 1;
        }
        
        energy
    }

    fn is_active(&self) -> bool {
        if self.recent_energy.len() < 3 {
            return false;
        }
        
        // Consider active if recent energy is consistently above noise floor
        let recent_avg: f32 = self.recent_energy.iter().rev().take(3).sum::<f32>() / 3.0;
        recent_avg > 0.003
    }
    
    fn speech_activity_ratio(&self) -> f32 {
        let total = self.high_energy_count + self.low_energy_count;
        if total == 0 {
            0.0
        } else {
            self.high_energy_count as f32 / total as f32
        }
    }
}

/// Zero crossing rate calculator
struct ZeroCrossingRateCalculator;

impl ZeroCrossingRateCalculator {
    fn calculate(samples: &[f32]) -> f32 {
        if samples.len() < 2 {
            return 0.0;
        }

        let mut crossings = 0;
        for i in 1..samples.len() {
            if (samples[i] >= 0.0 && samples[i - 1] < 0.0) || 
               (samples[i] < 0.0 && samples[i - 1] >= 0.0) {
                crossings += 1;
            }
        }

        crossings as f32 / (samples.len() - 1) as f32
    }
}

/// Pitch detector for voice activity
struct PitchDetector {
    window_size: usize,
    min_pitch: f32,
    max_pitch: f32,
}

impl PitchDetector {
    fn new(sample_rate: usize) -> Self {
        Self {
            window_size: sample_rate / 50, // 20ms window
            min_pitch: 80.0,  // Minimum human pitch (Hz)
            max_pitch: 400.0, // Maximum human pitch (Hz)
        }
    }

    fn detect(&self, samples: &[f32], sample_rate: f32) -> Option<f32> {
        if samples.len() < self.window_size {
            return None;
        }

        // Simple autocorrelation-based pitch detection
        let mut max_correlation = 0.0;
        let mut best_period = 0;
        
        let min_period = (sample_rate / self.max_pitch) as usize;
        let max_period = (sample_rate / self.min_pitch) as usize;
        
        for period in min_period..max_period.min(samples.len() / 2) {
            let mut correlation = 0.0;
            let mut count = 0;
            
            for i in 0..(samples.len() - period) {
                correlation += samples[i] * samples[i + period];
                count += 1;
            }
            
            if count > 0 {
                correlation /= count as f32;
                if correlation > max_correlation {
                    max_correlation = correlation;
                    best_period = period;
                }
            }
        }
        
        if max_correlation > 0.3 && best_period > 0 {
            Some(sample_rate / best_period as f32)
        } else {
            None
        }
    }
}

/// Pause detector for natural speech boundaries
struct PauseDetector {
    silence_threshold: f32,
    min_pause_duration_ms: u32,
    silence_frames: u32,
    frame_duration_ms: u32,
}

impl PauseDetector {
    fn new(frame_duration_ms: u32) -> Self {
        Self {
            silence_threshold: 0.001,
            min_pause_duration_ms: 200, // 200ms pause threshold
            silence_frames: 0,
            frame_duration_ms,
        }
    }

    fn detect_pauses(&mut self, energy: f32, zcr: f32) -> bool {
        // Consider it silence if both energy and ZCR are low
        let is_silence = energy < self.silence_threshold && zcr < 0.05;
        
        if is_silence {
            self.silence_frames += 1;
        } else {
            self.silence_frames = 0;
        }
        
        // Return true if we've had enough silence frames for a pause
        (self.silence_frames * self.frame_duration_ms) >= self.min_pause_duration_ms
    }
    
    fn reset(&mut self) {
        self.silence_frames = 0;
    }
}

/// Speech boundary detector
pub struct SpeechBoundaryDetector {
    energy_tracker: EnergyTracker,
    pitch_detector: PitchDetector,
    pause_detector: PauseDetector,
    frame_duration_ms: u32,
}

impl SpeechBoundaryDetector {
    pub fn new(sample_rate: usize, frame_duration_ms: u32) -> Self {
        Self {
            energy_tracker: EnergyTracker::new(10), // 10 frame window
            pitch_detector: PitchDetector::new(sample_rate),
            pause_detector: PauseDetector::new(frame_duration_ms),
            frame_duration_ms,
        }
    }

    pub fn detect_boundaries(&mut self, samples: &[f32]) -> BoundaryInfo {
        let energy = self.energy_tracker.calculate(samples);
        let zcr = ZeroCrossingRateCalculator::calculate(samples);
        let pitch = self.pitch_detector.detect(samples, 16000.0);
        
        // Detect pauses (potential sentence boundaries)
        let has_pause = self.pause_detector.detect_pauses(energy, zcr);
        
        // Simple heuristics for sentence boundaries
        let mut sentence_boundaries = Vec::new();
        let mut word_boundaries = Vec::new();
        
        if has_pause {
            sentence_boundaries.push(samples.len());
        }
        
        // Basic word boundary detection based on energy dips
        for i in (0..samples.len()).step_by(samples.len() / 10) {
            if i > 0 && samples[i].abs() < energy * 0.3 {
                word_boundaries.push(i);
            }
        }
        
        let is_complete_utterance = has_pause && self.energy_tracker.is_active();
        let speech_probability = self.energy_tracker.speech_activity_ratio();
        
        // Calculate confidence based on multiple factors
        let mut confidence = 0.5_f32; // Base confidence
        if pitch.is_some() {
            confidence += 0.3; // Pitch detected
        }
        if self.energy_tracker.is_active() {
            confidence += 0.2; // Energy activity
        }
        if speech_probability > 0.5 {
            confidence += 0.1; // Good speech ratio
        }
        
        BoundaryInfo {
            sentence_boundaries,
            word_boundaries,
            is_complete_utterance,
            confidence: confidence.min(1.0_f32),
            speech_probability,
        }
    }
    
    pub fn is_complete_utterance(&self, boundaries: &BoundaryInfo) -> bool {
        boundaries.is_complete_utterance && boundaries.confidence > 0.6
    }
}

/// Streaming VAD processor with persistent state
pub struct StreamingVadProcessor {
    config: StreamingVadConfig,
    boundary_detector: SpeechBoundaryDetector,
    noise_estimator: AdaptiveNoiseEstimator,
    frame_buffer: Vec<f32>,
    speech_buffer: VecDeque<Vec<f32>>,
    is_speaking: bool,
    speech_start_time: Option<Instant>,
    frame_count: u64,
    error_handler: Arc<ErrorHandler>,
}

impl StreamingVadProcessor {
    pub fn new(config: StreamingVadConfig) -> Result<Self> {
        let boundary_detector = SpeechBoundaryDetector::new(
            config.sample_rate, 
            config.frame_duration_ms
        );
        
        Ok(Self {
            boundary_detector,
            noise_estimator: AdaptiveNoiseEstimator::new(),
            frame_buffer: Vec::new(),
            speech_buffer: VecDeque::new(),
            is_speaking: false,
            speech_start_time: None,
            frame_count: 0,
            config,
            error_handler: Arc::new(ErrorHandler::new()),
        })
    }

    /// Process streaming audio with persistent state
    pub async fn process_stream(&mut self, samples: &[f32]) -> Result<StreamingResult> {
        if samples.is_empty() {
            return Ok(StreamingResult {
                speech_segments: Vec::new(),
                is_speaking: false,
                confidence: 0.0,
                boundary_info: BoundaryInfo {
                    sentence_boundaries: Vec::new(),
                    word_boundaries: Vec::new(),
                    is_complete_utterance: false,
                    confidence: 0.0,
                    speech_probability: 0.0,
                },
                noise_floor: self.noise_estimator.noise_floor(),
                energy_level: 0.0,
            });
        }

        // Update noise floor estimation
        self.noise_estimator.update(samples);

        // Calculate frame length in samples
        let frame_len = (self.config.sample_rate as f64 * (self.config.frame_duration_ms as f64 / 1000.0)) as usize;
        
        // Add samples to buffer
        self.frame_buffer.extend_from_slice(samples);
        
        let mut speech_segments = Vec::new();
        let mut final_boundary_info = BoundaryInfo {
            sentence_boundaries: Vec::new(),
            word_boundaries: Vec::new(),
            is_complete_utterance: false,
            confidence: 0.0,
            speech_probability: 0.0,
        };
        
        let mut total_energy = 0.0;
        let mut frame_count = 0;

        // Process complete frames
        while self.frame_buffer.len() >= frame_len {
            let frame: Vec<f32> = self.frame_buffer.drain(..frame_len).collect();
            
            match self.process_frame(&frame).await {
                Ok(result) => {
                    if !result.speech_segments.is_empty() {
                        speech_segments.extend(result.speech_segments);
                    }
                    
                    // Update boundary info with latest
                    final_boundary_info = result.boundary_info;
                    total_energy += result.energy_level;
                    frame_count += 1;
                    
                    self.frame_count += 1;
                }
                Err(e) => {
                    let error = AudioError::vad_processing_failed(frame.len(), &e.to_string());
                    let context = create_error_context("streaming_vad", "process_frame", None);
                    let _action = self.error_handler.handle_error(error, context).await;
                    
                    // Continue processing despite error
                    warn!("VAD frame processing error: {}", e);
                }
            }
        }
        
        let average_energy = if frame_count > 0 { total_energy / frame_count as f32 } else { 0.0 };
        
        Ok(StreamingResult {
            speech_segments,
            is_speaking: self.is_speaking,
            confidence: final_boundary_info.confidence,
            boundary_info: final_boundary_info,
            noise_floor: self.noise_estimator.noise_floor(),
            energy_level: average_energy,
        })
    }

    /// Process a single frame
    async fn process_frame(&mut self, frame: &[f32]) -> Result<StreamingResult> {
        // Detect speech boundaries
        let boundary_info = self.boundary_detector.detect_boundaries(frame);
        
        // Calculate energy metrics
        let energy = calculate_rms_energy(frame);
        let threshold = if self.config.adaptive_threshold {
            self.noise_estimator.adaptive_threshold()
        } else {
            self.config.energy_threshold
        };
        
        // Determine if this frame contains speech
        let has_speech = energy > threshold && boundary_info.speech_probability > 0.3;
        
        let mut speech_segments = Vec::new();
        
        // State machine for speech detection
        match (self.is_speaking, has_speech) {
            (false, true) => {
                // Start of speech
                self.is_speaking = true;
                self.speech_start_time = Some(Instant::now());
                
                // Add pre-speech padding if configured
                let pad_frames = (self.config.pre_speech_pad_ms as f32 / self.config.frame_duration_ms as f32) as usize;
                while self.speech_buffer.len() > pad_frames {
                    self.speech_buffer.pop_front();
                }
                
                // Add buffered frames as speech
                for buffered_frame in self.speech_buffer.drain(..) {
                    speech_segments.push(buffered_frame);
                }
                
                speech_segments.push(frame.to_vec());
                debug!("Speech started, frame {}", self.frame_count);
            }
            (true, true) => {
                // Continuation of speech
                speech_segments.push(frame.to_vec());
            }
            (true, false) => {
                // Potential end of speech, but keep in buffer for post-speech padding
                self.speech_buffer.push_back(frame.to_vec());
                
                // Check if we should end speech (after post-speech padding time)
                let pad_frames = (self.config.post_speech_pad_ms as f32 / self.config.frame_duration_ms as f32) as usize;
                if self.speech_buffer.len() > pad_frames {
                    // End of speech
                    self.is_speaking = false;
                    
                    // Check minimum speech duration
                    if let Some(start_time) = self.speech_start_time {
                        let duration = start_time.elapsed();
                        if duration >= Duration::from_millis(self.config.min_speech_duration_ms.into()) {
                            // Add post-speech padding
                            for buffered_frame in self.speech_buffer.drain(..) {
                                speech_segments.push(buffered_frame);
                            }
                            debug!("Speech ended, duration: {:?}, frame {}", duration, self.frame_count);
                        } else {
                            debug!("Speech too short ({:?}), discarding", duration);
                        }
                    }
                    
                    self.speech_start_time = None;
                    self.speech_buffer.clear();
                }
            }
            (false, false) => {
                // Silence, buffer frame for potential pre-speech padding
                self.speech_buffer.push_back(frame.to_vec());
                
                // Limit buffer size
                let max_buffer_frames = (self.config.pre_speech_pad_ms as f32 / self.config.frame_duration_ms as f32) as usize * 2;
                while self.speech_buffer.len() > max_buffer_frames {
                    self.speech_buffer.pop_front();
                }
            }
        }
        
        Ok(StreamingResult {
            speech_segments,
            is_speaking: self.is_speaking,
            confidence: boundary_info.confidence,
            boundary_info,
            noise_floor: self.noise_estimator.noise_floor(),
            energy_level: energy,
        })
    }

    /// Reset VAD state
    pub fn reset(&mut self) {
        self.is_speaking = false;
        self.speech_start_time = None;
        self.frame_buffer.clear();
        self.speech_buffer.clear();
        self.frame_count = 0;
        info!("StreamingVadProcessor reset");
    }
    
    /// Get current configuration
    pub fn config(&self) -> &StreamingVadConfig {
        &self.config
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: StreamingVadConfig) {
        self.config = config;
        // Reset to apply new configuration
        self.reset();
    }
    
    /// Get processing statistics
    pub fn get_statistics(&self) -> VadStatistics {
        VadStatistics {
            frames_processed: self.frame_count,
            current_noise_floor: self.noise_estimator.noise_floor(),
            is_currently_speaking: self.is_speaking,
            buffer_size: self.frame_buffer.len(),
            speech_buffer_size: self.speech_buffer.len(),
        }
    }
}

/// Statistics for monitoring VAD performance
#[derive(Debug, Clone, Serialize)]
pub struct VadStatistics {
    pub frames_processed: u64,
    pub current_noise_floor: f32,
    pub is_currently_speaking: bool,
    pub buffer_size: usize,
    pub speech_buffer_size: usize,
}

/// Helper function to calculate RMS energy
fn calculate_rms_energy(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    
    let sum_squares: f32 = samples.iter().map(|&x| x * x).sum();
    (sum_squares / samples.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_streaming_vad_basic() {
        let config = StreamingVadConfig::default();
        let mut vad = StreamingVadProcessor::new(config).unwrap();
        
        // Test with silence
        let silence = vec![0.0; 480]; // 30ms at 16kHz
        let result = vad.process_stream(&silence).await.unwrap();
        assert!(!result.is_speaking);
        
        // Test with speech-like signal
        let mut speech = vec![0.0; 480];
        for (i, sample) in speech.iter_mut().enumerate() {
            *sample = (i as f32 * 0.1).sin() * 0.1; // Simple sine wave
        }
        
        let result = vad.process_stream(&speech).await.unwrap();
        // May or may not be detected as speech on first frame
        // But should show some energy
        assert!(result.energy_level > 0.0);
    }

    #[test]
    fn test_boundary_detector() {
        let mut detector = SpeechBoundaryDetector::new(16000, 30);
        
        // Test with energy signal
        let samples: Vec<f32> = (0..480).map(|i| (i as f32 * 0.1).sin() * 0.1).collect();
        let boundaries = detector.detect_boundaries(&samples);
        
        assert!(boundaries.confidence > 0.0);
    }

    #[test]
    fn test_noise_estimator() {
        let mut estimator = AdaptiveNoiseEstimator::new();
        
        // Feed low-energy samples (noise)
        for _ in 0..10 {
            let noise: Vec<f32> = (0..480).map(|_| rand::random::<f32>() * 0.001).collect();
            estimator.update(&noise);
        }
        
        let noise_floor = estimator.noise_floor();
        assert!(noise_floor > 0.0);
        assert!(noise_floor < 0.01);
    }

    #[test]
    fn test_energy_tracker() {
        let mut tracker = EnergyTracker::new(5);
        
        // Test with high energy
        let high_energy: Vec<f32> = (0..480).map(|i| (i as f32 * 0.1).sin() * 0.1).collect();
        let energy = tracker.calculate(&high_energy);
        
        assert!(energy > 0.0);
        
        // Test activity detection after enough frames
        for _ in 0..3 {
            tracker.calculate(&high_energy);
        }
        
        // Should detect activity after consistent high energy
        assert!(tracker.is_active());
    }
}