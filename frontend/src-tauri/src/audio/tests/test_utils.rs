use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use rand::Rng;

/// Generate synthetic audio samples for testing
pub struct AudioTestGenerator {
    sample_rate: usize,
    duration_ms: u32,
}

impl AudioTestGenerator {
    pub fn new(sample_rate: usize, duration_ms: u32) -> Self {
        Self { sample_rate, duration_ms }
    }

    /// Generate pure silence
    pub fn generate_silence(&self) -> Vec<f32> {
        let num_samples = (self.sample_rate * self.duration_ms as usize) / 1000;
        vec![0.0; num_samples]
    }

    /// Generate white noise
    pub fn generate_noise(&self, amplitude: f32) -> Vec<f32> {
        let num_samples = (self.sample_rate * self.duration_ms as usize) / 1000;
        let mut rng = rand::thread_rng();
        (0..num_samples)
            .map(|_| rng.gen_range(-amplitude..amplitude))
            .collect()
    }

    /// Generate a sine wave at specified frequency
    pub fn generate_sine_wave(&self, frequency: f32, amplitude: f32) -> Vec<f32> {
        let num_samples = (self.sample_rate * self.duration_ms as usize) / 1000;
        let samples_per_cycle = self.sample_rate as f32 / frequency;
        
        (0..num_samples)
            .map(|i| {
                let phase = (i as f32 / samples_per_cycle) * 2.0 * std::f32::consts::PI;
                amplitude * phase.sin()
            })
            .collect()
    }

    /// Generate speech-like patterns (combination of frequencies)
    pub fn generate_speech_pattern(&self) -> Vec<f32> {
        let num_samples = (self.sample_rate * self.duration_ms as usize) / 1000;
        let mut rng = rand::thread_rng();
        
        // Simulate formants with multiple sine waves
        let formant1_freq = rng.gen_range(300.0..1000.0); // First formant
        let formant2_freq = rng.gen_range(1000.0..3000.0); // Second formant
        let formant3_freq = rng.gen_range(2000.0..4000.0); // Third formant
        
        let formant1 = self.generate_sine_wave(formant1_freq, 0.3);
        let formant2 = self.generate_sine_wave(formant2_freq, 0.2);
        let formant3 = self.generate_sine_wave(formant3_freq, 0.1);
        
        // Add some envelope modulation
        (0..num_samples)
            .map(|i| {
                let envelope = (i as f32 / num_samples as f32 * std::f32::consts::PI).sin();
                let sample = formant1[i] + formant2[i] + formant3[i];
                sample * envelope * 0.5
            })
            .collect()
    }

    /// Generate alternating speech and silence
    pub fn generate_speech_with_pauses(&self, speech_duration_ms: u32, pause_duration_ms: u32) -> Vec<f32> {
        let mut result = Vec::new();
        let mut current_time = 0;

        while current_time < self.duration_ms {
            // Add speech segment
            let speech_gen = AudioTestGenerator::new(self.sample_rate, speech_duration_ms.min(self.duration_ms - current_time));
            result.extend(speech_gen.generate_speech_pattern());
            current_time += speech_duration_ms;

            if current_time >= self.duration_ms {
                break;
            }

            // Add pause segment
            let pause_gen = AudioTestGenerator::new(self.sample_rate, pause_duration_ms.min(self.duration_ms - current_time));
            result.extend(pause_gen.generate_silence());
            current_time += pause_duration_ms;
        }

        result
    }

    /// Generate audio with gradually changing properties
    pub fn generate_dynamic_audio(&self) -> Vec<f32> {
        let num_samples = (self.sample_rate * self.duration_ms as usize) / 1000;
        let mut result = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let progress = i as f32 / num_samples as f32;
            
            // Frequency sweep from 200Hz to 2000Hz
            let frequency = 200.0 + (1800.0 * progress);
            
            // Amplitude envelope (fade in, sustain, fade out)
            let amplitude = if progress < 0.1 {
                progress * 10.0 // Fade in
            } else if progress > 0.9 {
                (1.0 - progress) * 10.0 // Fade out
            } else {
                1.0 // Sustain
            };

            let phase = (i as f32 / self.sample_rate as f32) * frequency * 2.0 * std::f32::consts::PI;
            let sample = amplitude * 0.3 * phase.sin();
            
            result.push(sample);
        }

        result
    }

    /// Generate realistic conversation audio (alternating speakers)
    pub fn generate_conversation(&self) -> (Vec<f32>, Vec<f32>) {
        let segment_duration = self.duration_ms / 4; // Divide into segments
        let mut speaker1_audio = Vec::new();
        let mut speaker2_audio = Vec::new();

        for i in 0..4 {
            let segment_gen = AudioTestGenerator::new(self.sample_rate, segment_duration);
            
            if i % 2 == 0 {
                // Speaker 1 talks, Speaker 2 silent
                speaker1_audio.extend(segment_gen.generate_speech_pattern());
                speaker2_audio.extend(segment_gen.generate_silence());
            } else {
                // Speaker 2 talks, Speaker 1 silent
                speaker1_audio.extend(segment_gen.generate_silence());
                speaker2_audio.extend(segment_gen.generate_speech_pattern());
            }
        }

        (speaker1_audio, speaker2_audio)
    }

    /// Add realistic audio artifacts
    pub fn add_artifacts(&self, samples: &mut [f32]) {
        let mut rng = rand::thread_rng();
        
        for sample in samples.iter_mut() {
            // Add small amount of noise
            *sample += rng.gen_range(-0.01..0.01);
            
            // Occasional pops/clicks (simulate real-world audio)
            if rng.gen_bool(0.001) { // 0.1% chance
                *sample += rng.gen_range(-0.1..0.1);
            }
            
            // Clamp to valid range
            *sample = sample.clamp(-1.0, 1.0);
        }
    }
}

/// Validate audio samples are within expected range
pub fn validate_audio_samples(samples: &[f32]) -> bool {
    samples.iter().all(|&s| s >= -1.0 && s <= 1.0 && !s.is_nan() && !s.is_infinite())
}

/// Calculate RMS energy of audio samples
pub fn calculate_rms_energy(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    
    let sum_squares: f32 = samples.iter().map(|&s| s * s).sum();
    (sum_squares / samples.len() as f32).sqrt()
}

/// Calculate zero-crossing rate
pub fn calculate_zero_crossing_rate(samples: &[f32]) -> f32 {
    if samples.len() < 2 {
        return 0.0;
    }
    
    let crossings = samples.windows(2)
        .filter(|window| window[0] * window[1] < 0.0)
        .count();
    
    crossings as f32 / (samples.len() - 1) as f32
}

/// Calculate spectral centroid (rough pitch estimation)
pub fn calculate_spectral_centroid(samples: &[f32], sample_rate: usize) -> f32 {
    // Simplified spectral centroid calculation
    // In real implementation, would use FFT
    
    let mut weighted_sum = 0.0;
    let mut magnitude_sum = 0.0;
    
    // Use autocorrelation to estimate fundamental frequency
    let max_lag = sample_rate / 50; // Minimum 50Hz
    let mut max_correlation = 0.0;
    let mut best_lag = 0;
    
    for lag in sample_rate / 800..max_lag { // Between 800Hz and 50Hz
        let mut correlation = 0.0;
        for i in lag..samples.len() {
            correlation += samples[i] * samples[i - lag];
        }
        
        if correlation > max_correlation {
            max_correlation = correlation;
            best_lag = lag;
        }
    }
    
    if best_lag > 0 {
        sample_rate as f32 / best_lag as f32
    } else {
        0.0
    }
}

/// Performance measurement utilities
pub struct PerformanceMeter {
    start_time: std::time::Instant,
}

impl PerformanceMeter {
    pub fn start() -> Self {
        Self {
            start_time: std::time::Instant::now(),
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    pub fn elapsed_us(&self) -> u64 {
        self.start_time.elapsed().as_micros() as u64
    }

    pub fn check_performance(&self, operation: &str, max_latency_ms: u64) -> bool {
        let elapsed = self.elapsed_ms();
        let passed = elapsed <= max_latency_ms;
        
        if passed {
            println!("✓ {}: {}ms (within {}ms limit)", operation, elapsed, max_latency_ms);
        } else {
            println!("✗ {}: {}ms (exceeded {}ms limit)", operation, elapsed, max_latency_ms);
        }
        
        passed
    }
}

/// Memory usage tracking
pub struct MemoryTracker {
    initial_memory: usize,
}

impl MemoryTracker {
    pub fn start() -> Self {
        Self {
            initial_memory: get_current_memory_usage(),
        }
    }

    pub fn memory_delta_mb(&self) -> f64 {
        let current = get_current_memory_usage();
        (current as f64 - self.initial_memory as f64) / (1024.0 * 1024.0)
    }

    pub fn check_memory_usage(&self, operation: &str, max_memory_mb: f64) -> bool {
        let delta = self.memory_delta_mb();
        let passed = delta <= max_memory_mb;
        
        if passed {
            println!("✓ {}: {:.2}MB memory increase (within {:.2}MB limit)", operation, delta, max_memory_mb);
        } else {
            println!("✗ {}: {:.2}MB memory increase (exceeded {:.2}MB limit)", operation, delta, max_memory_mb);
        }
        
        passed
    }
}

#[cfg(target_os = "macos")]
fn get_current_memory_usage() -> usize {
    use std::mem;
    use std::ptr;

    unsafe {
        let mut info: libc::mach_task_basic_info = mem::zeroed();
        let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
        let result = libc::task_info(
            libc::mach_task_self(),
            libc::MACH_TASK_BASIC_INFO,
            &mut info as *mut _ as *mut libc::integer_t,
            &mut count,
        );
        
        if result == libc::KERN_SUCCESS {
            info.resident_size as usize
        } else {
            0
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn get_current_memory_usage() -> usize {
    // Fallback implementation for other platforms
    0
}

/// Test assertion helpers
pub fn assert_audio_quality(samples: &[f32], min_rms: f32, max_rms: f32, description: &str) {
    assert!(validate_audio_samples(samples), "{}: Audio samples contain invalid values", description);
    
    let rms = calculate_rms_energy(samples);
    assert!(rms >= min_rms && rms <= max_rms, 
           "{}: RMS energy {:.6} not in expected range [{:.6}, {:.6}]", 
           description, rms, min_rms, max_rms);
}

pub fn assert_processing_latency(elapsed_ms: u64, max_latency_ms: u64, operation: &str) {
    assert!(elapsed_ms <= max_latency_ms, 
           "{}: Processing took {}ms, exceeded maximum {}ms", 
           operation, elapsed_ms, max_latency_ms);
}

pub fn assert_transcription_quality(text: &str, min_length: usize, max_length: usize, description: &str) {
    assert!(!text.is_empty(), "{}: Transcription is empty", description);
    assert!(text.len() >= min_length && text.len() <= max_length,
           "{}: Transcription length {} not in expected range [{}, {}]",
           description, text.len(), min_length, max_length);
    
    // Check for common transcription artifacts
    assert!(!text.contains("???"), "{}: Transcription contains uncertainty markers", description);
    assert!(!text.chars().all(|c| c.is_whitespace()), "{}: Transcription is only whitespace", description);
}

/// Async test utilities
pub async fn wait_for_condition<F, Fut>(mut condition: F, timeout_ms: u64, check_interval_ms: u64) -> Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = std::time::Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let interval = Duration::from_millis(check_interval_ms);
    
    loop {
        if condition().await {
            return Ok(());
        }
        
        if start.elapsed() > timeout {
            return Err(anyhow::anyhow!("Condition not met within {}ms timeout", timeout_ms));
        }
        
        tokio::time::sleep(interval).await;
    }
}

/// Channel testing helpers
pub async fn send_test_audio<T>(channel: &Arc<super::super::ManagedChannel<T>>, data: T) -> Result<()>
where
    T: Clone + Send + Sync + 'static,
{
    channel.send(data).await
}

pub async fn receive_test_audio<T>(channel: &Arc<super::super::ManagedChannel<T>>, timeout_ms: u64) -> Result<T>
where
    T: Clone + Send + Sync + 'static,
{
    let mut receiver = channel.subscribe().await?;
    
    tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        receiver.recv()
    ).await
    .map_err(|_| anyhow::anyhow!("Timeout waiting for audio data"))?
    .map_err(|e| anyhow::anyhow!("Channel receive error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_generation() {
        let generator = AudioTestGenerator::new(16000, 1000); // 1 second at 16kHz
        
        // Test silence generation
        let silence = generator.generate_silence();
        assert_eq!(silence.len(), 16000);
        assert!(silence.iter().all(|&s| s == 0.0));
        
        // Test sine wave generation
        let sine = generator.generate_sine_wave(440.0, 0.5);
        assert_eq!(sine.len(), 16000);
        assert!(validate_audio_samples(&sine));
        
        // Test RMS calculation
        let rms = calculate_rms_energy(&sine);
        assert!(rms > 0.3 && rms < 0.4); // Should be around 0.35 for 0.5 amplitude sine
        
        // Test speech pattern
        let speech = generator.generate_speech_pattern();
        assert_eq!(speech.len(), 16000);
        assert!(validate_audio_samples(&speech));
        assert!(calculate_rms_energy(&speech) > 0.01); // Should have some energy
    }

    #[test]
    fn test_audio_analysis() {
        let generator = AudioTestGenerator::new(16000, 1000);
        
        // Test with sine wave
        let sine = generator.generate_sine_wave(440.0, 0.5);
        let zcr = calculate_zero_crossing_rate(&sine);
        assert!(zcr > 0.02 && zcr < 0.08); // Should have reasonable zero crossings
        
        // Test spectral centroid
        let centroid = calculate_spectral_centroid(&sine, 16000);
        assert!(centroid > 300.0 && centroid < 600.0); // Should be around 440Hz
    }

    #[test]
    fn test_performance_meter() {
        let meter = PerformanceMeter::start();
        std::thread::sleep(Duration::from_millis(10));
        
        let elapsed = meter.elapsed_ms();
        assert!(elapsed >= 10 && elapsed < 50); // Should be around 10ms with some tolerance
    }
}