use anyhow::{anyhow, Result};
use silero_rs::{VadConfig, VadSession, VadTransition};
use log::{debug, info, warn};
use std::time::Duration;
use std::sync::Arc;

use super::streaming_vad::{StreamingVadProcessor, StreamingVadConfig, StreamingResult, VadStatistics};
use super::error::{AudioError, ErrorHandler, create_error_context};
use serde::Serialize;

/// Advanced VAD with dual-channel support using streaming VAD
pub struct DualChannelVad {
    mic_vad: StreamingVadProcessor,
    speaker_vad: StreamingVadProcessor,
    mixed_vad: StreamingVadProcessor,
    error_handler: Arc<ErrorHandler>,
}

impl DualChannelVad {
    pub fn new(sample_rate: usize) -> Result<Self> {
        let config = StreamingVadConfig {
            sample_rate,
            frame_duration_ms: 30,
            redemption_time_ms: 800, // Increased - keeps speech segments together longer
            pre_speech_pad_ms: 300,  // Increased - more context before speech
            post_speech_pad_ms: 500, // Increased - more context after speech  
            min_speech_duration_ms: 500, // Increased - prevents very short segments
            adaptive_threshold: true,
            energy_threshold: 0.002, // Slightly reduced - less aggressive
            zero_crossing_threshold: 0.15, // Increased - more tolerant of speech variations
            pitch_detection_enabled: true,
        };

        Ok(Self {
            mic_vad: StreamingVadProcessor::new(config.clone())?,
            speaker_vad: StreamingVadProcessor::new(config.clone())?,
            mixed_vad: StreamingVadProcessor::new(config)?,
            error_handler: Arc::new(ErrorHandler::new()),
        })
    }

    /// Process dual-channel audio with streaming VAD
    pub async fn process_dual_channel(&mut self, mic_samples: &[f32], speaker_samples: &[f32]) -> Result<Vec<f32>> {
        let mut final_speech: Vec<f32> = Vec::new();
        
        // Process microphone audio with streaming VAD
        if !mic_samples.is_empty() {
            match self.mic_vad.process_stream(mic_samples).await {
                Ok(result) => {
                    for speech_segment in result.speech_segments {
                        final_speech.extend(speech_segment);
                    }
                    debug!("Mic VAD: {} -> {} speech samples (confidence: {:.2})", 
                           mic_samples.len(), final_speech.len(), result.confidence);
                }
                Err(e) => {
                    let error = AudioError::vad_processing_failed(mic_samples.len(), &e.to_string());
                    let context = create_error_context("dual_channel_vad", "process_mic", None);
                    let _action = self.error_handler.handle_error(error, context).await;
                    
                    warn!("Mic VAD processing failed: {}, using fallback", e);
                    // Fallback: use original samples if they have sufficient energy
                    let energy = mic_samples.iter().map(|&x| x * x).sum::<f32>() / mic_samples.len() as f32;
                    if energy > 0.003 {
                        final_speech.extend_from_slice(mic_samples);
                    }
                }
            }
        }

        // Process speaker audio with streaming VAD
        if !speaker_samples.is_empty() {
            let mut speaker_speech = Vec::new();
            match self.speaker_vad.process_stream(speaker_samples).await {
                Ok(result) => {
                    for speech_segment in result.speech_segments {
                        speaker_speech.extend(speech_segment);
                    }
                    debug!("Speaker VAD: {} -> {} speech samples (confidence: {:.2})", 
                           speaker_samples.len(), speaker_speech.len(), result.confidence);
                }
                Err(e) => {
                    let error = AudioError::vad_processing_failed(speaker_samples.len(), &e.to_string());
                    let context = create_error_context("dual_channel_vad", "process_speaker", None);
                    let _action = self.error_handler.handle_error(error, context).await;
                    
                    warn!("Speaker VAD processing failed: {}, using fallback", e);
                    // Fallback: use original samples if they have sufficient energy
                    let energy = speaker_samples.iter().map(|&x| x * x).sum::<f32>() / speaker_samples.len() as f32;
                    if energy > 0.003 {
                        speaker_speech.extend_from_slice(speaker_samples);
                    }
                }
            }
            final_speech.extend(speaker_speech);
        }

        // If we have both channels, also process mixed audio for better results
        if !mic_samples.is_empty() && !speaker_samples.is_empty() {
            let mixed_audio = self.mix_channels(mic_samples, speaker_samples);
            
            match self.mixed_vad.process_stream(&mixed_audio).await {
                Ok(result) => {
                    // Only use mixed results if they have higher confidence
                    if result.confidence > 0.7 && !result.speech_segments.is_empty() {
                        debug!("Using mixed audio VAD result (confidence: {:.2})", result.confidence);
                        final_speech.clear(); // Replace with mixed results
                        for speech_segment in result.speech_segments {
                            final_speech.extend(speech_segment);
                        }
                    }
                }
                Err(e) => {
                    debug!("Mixed audio VAD failed: {}, using individual channel results", e);
                }
            }
        }

        Ok(final_speech)
    }

    /// Mix two audio channels with intelligent gain control
    fn mix_channels(&self, mic_samples: &[f32], speaker_samples: &[f32]) -> Vec<f32> {
        let max_len = mic_samples.len().max(speaker_samples.len());
        let mut mixed_audio = Vec::with_capacity(max_len);
        
        // Calculate RMS for dynamic mixing
        let mic_rms = if !mic_samples.is_empty() {
            (mic_samples.iter().map(|&x| x * x).sum::<f32>() / mic_samples.len() as f32).sqrt()
        } else {
            0.0
        };
        
        let speaker_rms = if !speaker_samples.is_empty() {
            (speaker_samples.iter().map(|&x| x * x).sum::<f32>() / speaker_samples.len() as f32).sqrt()
        } else {
            0.0
        };
        
        // Dynamic gain adjustment based on signal strength
        let (mic_gain, speaker_gain) = if mic_rms > speaker_rms * 2.0 {
            (0.8, 0.4) // Mic is much stronger, reduce speaker
        } else if speaker_rms > mic_rms * 2.0 {
            (0.4, 0.8) // Speaker is much stronger, reduce mic
        } else {
            (0.6, 0.7) // Balanced mixing
        };
        
        for i in 0..max_len {
            let mic_sample = mic_samples.get(i).copied().unwrap_or(0.0);
            let speaker_sample = speaker_samples.get(i).copied().unwrap_or(0.0);
            
            // Mix with dynamic gain and prevent clipping
            let mixed_sample = (mic_sample * mic_gain + speaker_sample * speaker_gain).clamp(-1.0, 1.0);
            mixed_audio.push(mixed_sample);
        }
        
        mixed_audio
    }

    /// Reset all VAD processors
    pub fn reset(&mut self) {
        self.mic_vad.reset();
        self.speaker_vad.reset();
        self.mixed_vad.reset();
    }

    /// Get VAD statistics for monitoring
    pub fn get_statistics(&self) -> DualChannelVadStats {
        DualChannelVadStats {
            mic_stats: self.mic_vad.get_statistics(),
            speaker_stats: self.speaker_vad.get_statistics(),
            mixed_stats: self.mixed_vad.get_statistics(),
        }
    }
}

/// Statistics for dual-channel VAD monitoring
#[derive(Debug, Clone, Serialize)]
pub struct DualChannelVadStats {
    pub mic_stats: VadStatistics,
    pub speaker_stats: VadStatistics,
    pub mixed_stats: VadStatistics,
}


/// Runs a quick Silero VAD over a mono 16kHz buffer.
/// Returns concatenated speech-only samples if any speech is detected,
/// otherwise returns an empty Vec to indicate no speech.
pub fn extract_speech_16k(samples_mono_16k: &[f32]) -> Result<Vec<f32>> {
    let mut config = VadConfig::default();
    config.sample_rate = 16_000usize;
    // Very lenient settings to avoid filtering out speech
    config.redemption_time = std::time::Duration::from_millis(50);      // Very short redemption
    config.pre_speech_pad = std::time::Duration::from_millis(100);      // Short pre-pad
    config.post_speech_pad = std::time::Duration::from_millis(25);      // Short post-pad
    config.min_speech_time = std::time::Duration::from_millis(5);       // Very low minimum

    let mut session = VadSession::new(config).map_err(|_| anyhow!("VadSessionCreationFailed"))?;

    // Process in 30ms frames (480 samples @ 16kHz)
    let frame_len = 480usize;
    let mut speech_out: Vec<f32> = Vec::new();
    let mut in_speech = false;
    let mut speech_start_idx = 0;

    debug!("VAD: Processing {} samples in {} frames", samples_mono_16k.len(), samples_mono_16k.len() / frame_len);

    for (frame_idx, frame) in samples_mono_16k.chunks(frame_len).enumerate() {
        if frame.is_empty() { continue; }
        
        let transitions = session.process(frame)
            .map_err(|e| anyhow!("VadProcessingFailed: {}", e))?;
        
        for t in transitions {
            match t {
                VadTransition::SpeechStart { .. } => {
                    debug!("VAD: Speech started at frame {}", frame_idx);
                    in_speech = true;
                    speech_start_idx = frame_idx * frame_len;
                }
                VadTransition::SpeechEnd { samples, .. } => {
                    debug!("VAD: Speech ended at frame {}, collected {} samples", frame_idx, samples.len());
                    in_speech = false;
                    // Add the samples from this transition
                    if !samples.is_empty() {
                        speech_out.extend_from_slice(&samples);
                    }
                    // Also add any samples we collected during speech
                    let speech_end_idx = (frame_idx + 1) * frame_len;
                    if speech_start_idx < speech_end_idx {
                        let collected_samples = &samples_mono_16k[speech_start_idx..speech_end_idx];
                        speech_out.extend_from_slice(collected_samples);
                    }
                }
            }
        }
        
        // If we're in speech, collect this frame's samples
        if in_speech {
            speech_out.extend_from_slice(frame);
        }
    }

    debug!("VAD: Input {} samples, output {} speech samples", 
          samples_mono_16k.len(), speech_out.len());
    
    // Adaptive threshold based on input audio levels
    let input_avg_level = samples_mono_16k.iter().map(|&x| x.abs()).sum::<f32>() / samples_mono_16k.len() as f32;
    
    if speech_out.len() < frame_len / 32 { // Super lenient - only 1/32 of a frame (15 samples)
        // If input has very low levels, it's probably silence - skip it
        if input_avg_level < 0.001 {
            debug!("VAD: Very low input levels ({:.6}), skipping silent chunk", input_avg_level);
            return Ok(Vec::new());
        } else {
            // Input has some audio but VAD didn't detect speech - include it anyway
            // This prevents losing audio during VAD false negatives
            debug!("VAD: Input has audio ({:.6}) but VAD detected no speech, including input anyway", input_avg_level);
            return Ok(samples_mono_16k.to_vec());
        }
    }

    Ok(speech_out)
}

 