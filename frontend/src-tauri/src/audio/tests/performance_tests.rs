use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use anyhow::Result;
use futures::future::join_all;

use super::test_utils::*;
use super::super::{
    DualChannelVad, IntelligentChunker, ChunkingConfig, StreamingWhisperService,
    StreamingWhisperConfig, ManagedChannel, RecoveryStrategy, AdaptiveBuffer, OverflowStrategy,
};

/// Performance test suite focusing on latency, throughput, and resource usage
#[cfg(test)]
mod tests {
    use super::*;

    /// Test end-to-end latency requirements
    #[tokio::test]
    async fn test_end_to_end_latency() -> Result<()> {
        println!("🚀 Testing end-to-end latency requirements");
        
        let sample_rate = 16000;
        let generator = AudioTestGenerator::new(sample_rate, 1000); // 1 second chunks
        
        // Performance targets
        const MAX_VAD_LATENCY_MS: u64 = 50;
        const MAX_CHUNKING_LATENCY_MS: u64 = 30;
        const MAX_TOTAL_PIPELINE_LATENCY_MS: u64 = 200;
        
        // Generate test audio
        let test_audio = generator.generate_speech_pattern();
        let mut total_pipeline_time = 0u64;
        
        // Test VAD latency
        let mut vad = DualChannelVad::new(sample_rate)?;
        let vad_timer = PerformanceMeter::start();
        let vad_result = vad.process_dual_channel(&test_audio, &[]).await?;
        let vad_latency = vad_timer.elapsed_ms();
        total_pipeline_time += vad_latency;
        
        assert_processing_latency(vad_latency, MAX_VAD_LATENCY_MS, "VAD processing");
        
        // Test chunking latency
        let chunker_config = ChunkingConfig {
            sample_rate,
            min_chunk_duration_ms: 500,
            max_chunk_duration_ms: 3000,
            silence_threshold_ms: 200,
            overlap_duration_ms: 100,
            adaptive_chunking: true,
            preserve_word_boundaries: true,
        };
        
        let mut chunker = IntelligentChunker::new(chunker_config)?;
        let chunking_timer = PerformanceMeter::start();
        let chunk_result = chunker.process_stream(&vad_result).await?;
        let chunking_latency = chunking_timer.elapsed_ms();
        total_pipeline_time += chunking_latency;
        
        assert_processing_latency(chunking_latency, MAX_CHUNKING_LATENCY_MS, "Intelligent chunking");
        
        // Test total pipeline latency
        assert_processing_latency(total_pipeline_time, MAX_TOTAL_PIPELINE_LATENCY_MS, "Total pipeline");
        
        println!("✅ Latency breakdown:");
        println!("   VAD: {}ms (limit: {}ms)", vad_latency, MAX_VAD_LATENCY_MS);
        println!("   Chunking: {}ms (limit: {}ms)", chunking_latency, MAX_CHUNKING_LATENCY_MS);
        println!("   Total: {}ms (limit: {}ms)", total_pipeline_time, MAX_TOTAL_PIPELINE_LATENCY_MS);
        
        Ok(())
    }

    /// Test throughput under sustained load
    #[tokio::test]
    async fn test_sustained_throughput() -> Result<()> {
        println!("🚀 Testing sustained throughput");
        
        const DURATION_SECONDS: u64 = 10;
        const CHUNK_SIZE_MS: u32 = 100; // Process 100ms chunks
        const TARGET_THROUGHPUT_CHUNKS_PER_SEC: u64 = 10; // 10 chunks/second = 1 second of audio per second
        
        let sample_rate = 16000;
        let generator = AudioTestGenerator::new(sample_rate, CHUNK_SIZE_MS);
        let test_chunk = generator.generate_speech_pattern();
        
        let mut vad = DualChannelVad::new(sample_rate)?;
        let start_time = Instant::now();
        let mut chunks_processed = 0u64;
        let memory_tracker = MemoryTracker::start();
        
        // Process chunks continuously for the test duration
        while start_time.elapsed().as_secs() < DURATION_SECONDS {
            let chunk_start = Instant::now();
            
            // Process the chunk
            let _result = vad.process_dual_channel(&test_chunk, &[]).await?;
            chunks_processed += 1;
            
            // Maintain target rate (sleep to prevent overwhelming the system)
            let target_chunk_duration = Duration::from_millis(1000 / TARGET_THROUGHPUT_CHUNKS_PER_SEC);
            let elapsed = chunk_start.elapsed();
            if elapsed < target_chunk_duration {
                tokio::time::sleep(target_chunk_duration - elapsed).await;
            }
        }
        
        let actual_duration = start_time.elapsed();
        let actual_throughput = chunks_processed as f64 / actual_duration.as_secs_f64();
        
        // Validate throughput
        assert!(actual_throughput >= TARGET_THROUGHPUT_CHUNKS_PER_SEC as f64 * 0.9, 
               "Throughput {:.1} chunks/sec below 90% of target {}", 
               actual_throughput, TARGET_THROUGHPUT_CHUNKS_PER_SEC);
        
        // Check memory usage didn't grow excessively
        assert!(memory_tracker.check_memory_usage("Sustained throughput", 50.0),
               "Memory usage should stay reasonable during sustained load");
        
        println!("✅ Sustained throughput test:");
        println!("   Processed {} chunks in {:.1}s", chunks_processed, actual_duration.as_secs_f64());
        println!("   Throughput: {:.1} chunks/sec (target: {})", actual_throughput, TARGET_THROUGHPUT_CHUNKS_PER_SEC);
        
        Ok(())
    }

    /// Test concurrent processing performance
    #[tokio::test]
    async fn test_concurrent_processing_performance() -> Result<()> {
        println!("🚀 Testing concurrent processing performance");
        
        const NUM_CONCURRENT_STREAMS: usize = 8;
        const CHUNKS_PER_STREAM: usize = 10;
        const MAX_CONCURRENT_PROCESSING_TIME_MS: u64 = 3000;
        
        let sample_rate = 16000;
        let generator = AudioTestGenerator::new(sample_rate, 1000);
        let test_audio = generator.generate_speech_pattern();
        
        let memory_tracker = MemoryTracker::start();
        let performance_timer = PerformanceMeter::start();
        
        // Create concurrent processing tasks
        let mut handles = Vec::new();
        
        for stream_id in 0..NUM_CONCURRENT_STREAMS {
            let audio_copy = test_audio.clone();
            
            let handle = tokio::spawn(async move {
                let mut vad = DualChannelVad::new(sample_rate).unwrap();
                let mut chunks_processed = 0;
                let stream_start = Instant::now();
                
                for chunk_id in 0..CHUNKS_PER_STREAM {
                    let chunk_timer = PerformanceMeter::start();
                    let _result = vad.process_dual_channel(&audio_copy, &[]).await.unwrap();
                    let chunk_latency = chunk_timer.elapsed_ms();
                    
                    chunks_processed += 1;
                    
                    // Log slow chunks
                    if chunk_latency > 100 {
                        println!("   Stream {}, chunk {}: {}ms (slow)", stream_id, chunk_id, chunk_latency);
                    }
                }
                
                let stream_duration = stream_start.elapsed();
                (stream_id, chunks_processed, stream_duration.as_millis() as u64)
            });
            
            handles.push(handle);
        }
        
        // Wait for all streams to complete
        let results = join_all(handles).await;
        let total_processing_time = performance_timer.elapsed_ms();
        
        // Validate results
        let mut total_chunks = 0;
        for result in results {
            let (stream_id, chunks, duration_ms) = result?;
            total_chunks += chunks;
            println!("   Stream {}: {} chunks in {}ms", stream_id, chunks, duration_ms);
        }
        
        // Performance validation
        assert_processing_latency(total_processing_time, MAX_CONCURRENT_PROCESSING_TIME_MS, 
                                 "Concurrent processing");
        
        // Memory validation
        assert!(memory_tracker.check_memory_usage("Concurrent processing", 200.0),
               "Memory usage should be reasonable for concurrent streams");
        
        println!("✅ Concurrent processing results:");
        println!("   {} streams processed {} total chunks in {}ms", 
                NUM_CONCURRENT_STREAMS, total_chunks, total_processing_time);
        println!("   Average: {:.1} chunks/stream", total_chunks as f64 / NUM_CONCURRENT_STREAMS as f64);
        
        Ok(())
    }

    /// Test memory usage patterns and potential leaks
    #[tokio::test]
    async fn test_memory_usage_patterns() -> Result<()> {
        println!("🚀 Testing memory usage patterns");
        
        let sample_rate = 16000;
        let generator = AudioTestGenerator::new(sample_rate, 2000); // 2 second chunks
        let base_memory = MemoryTracker::start();
        
        // Test 1: Buffer memory usage
        {
            let buffer_memory = MemoryTracker::start();
            let buffer = AdaptiveBuffer::new(1000, 10000, OverflowStrategy::DropOldest);
            
            // Fill buffer with data
            for i in 0..5000 {
                let data = vec![i as f32; 1000]; // 1000 samples each
                buffer.push(data).await.ok(); // Ignore overflow errors
            }
            
            assert!(buffer_memory.check_memory_usage("AdaptiveBuffer fill", 100.0),
                   "Buffer memory usage should be bounded");
            
            // Clear buffer and check memory release
            buffer.clear().await;
            tokio::time::sleep(Duration::from_millis(100)).await; // Allow GC
            
            println!("   Buffer memory after clear: {:.2}MB delta", buffer_memory.memory_delta_mb());
        }
        
        // Test 2: VAD processor memory usage over time
        {
            let vad_memory = MemoryTracker::start();
            let mut vad = DualChannelVad::new(sample_rate)?;
            
            // Process many chunks to check for memory leaks
            for i in 0..50 {
                let test_audio = generator.generate_speech_pattern();
                let _result = vad.process_dual_channel(&test_audio, &[]).await?;
                
                // Check memory every 10 iterations
                if i % 10 == 9 {
                    let current_usage = vad_memory.memory_delta_mb();
                    println!("   VAD memory after {} iterations: {:.2}MB", i + 1, current_usage);
                    
                    // Memory shouldn't grow excessively
                    assert!(current_usage < 50.0, "VAD memory usage {} exceeds 50MB", current_usage);
                }
            }
            
            // Final memory check
            assert!(vad_memory.check_memory_usage("VAD processing loop", 30.0),
                   "VAD should not leak significant memory");
        }
        
        // Test 3: Channel memory usage with high throughput
        {
            let channel_memory = MemoryTracker::start();
            let channel = Arc::new(ManagedChannel::new(
                1000,
                RecoveryStrategy::ExponentialBackoff {
                    base_delay_ms: 10,
                    max_delay_ms: 100,
                    max_retries: 3,
                },
                "memory_test_channel".to_string(),
            ));
            
            // High-throughput data transmission
            let mut handles = Vec::new();
            for _ in 0..10 {
                let channel_clone = Arc::clone(&channel);
                let handle = tokio::spawn(async move {
                    for i in 0..100 {
                        let data = vec![i as f32; 500];
                        let _ = channel_clone.send(data).await; // Some may fail due to overflow
                    }
                });
                handles.push(handle);
            }
            
            // Wait for all senders
            join_all(handles).await;
            
            assert!(channel_memory.check_memory_usage("Channel high-throughput", 20.0),
                   "Channel should handle high throughput without excessive memory use");
        }
        
        // Overall memory check
        assert!(base_memory.check_memory_usage("Complete memory test", 150.0),
               "Overall memory usage should be reasonable");
        
        println!("✅ Memory usage patterns validated");
        Ok(())
    }

    /// Test performance scaling with different audio configurations
    #[tokio::test]
    async fn test_performance_scaling() -> Result<()> {
        println!("🚀 Testing performance scaling");
        
        let sample_rates = vec![8000, 16000, 22050, 44100];
        let chunk_durations = vec![500, 1000, 2000, 5000]; // milliseconds
        
        let mut results = Vec::new();
        
        for &sample_rate in &sample_rates {
            for &chunk_duration in &chunk_durations {
                let generator = AudioTestGenerator::new(sample_rate, chunk_duration);
                let test_audio = generator.generate_speech_pattern();
                
                let mut vad = DualChannelVad::new(sample_rate)?;
                let timer = PerformanceMeter::start();
                
                let _result = vad.process_dual_channel(&test_audio, &[]).await?;
                
                let processing_time = timer.elapsed_ms();
                let samples_per_ms = test_audio.len() as f64 / processing_time as f64;
                
                results.push((sample_rate, chunk_duration, processing_time, samples_per_ms));
                
                println!("   {}Hz, {}ms chunk: {}ms processing ({:.0} samples/ms)", 
                        sample_rate, chunk_duration, processing_time, samples_per_ms);
            }
        }
        
        // Analyze scaling characteristics
        println!("✅ Performance scaling analysis:");
        
        // Find best and worst performing configurations
        results.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap()); // Sort by samples/ms descending
        
        let best = &results[0];
        let worst = &results[results.len() - 1];
        
        println!("   Best: {}Hz, {}ms chunk ({:.0} samples/ms)", best.0, best.1, best.3);
        println!("   Worst: {}Hz, {}ms chunk ({:.0} samples/ms)", worst.0, worst.1, worst.3);
        
        // Validate minimum performance threshold
        assert!(worst.3 > 100.0, "Even worst case should process >100 samples/ms");
        
        Ok(())
    }

    /// Test buffer performance under various strategies
    #[tokio::test]
    async fn test_buffer_performance_strategies() -> Result<()> {
        println!("🚀 Testing buffer performance strategies");
        
        const BUFFER_SIZE: usize = 1000;
        const MAX_SIZE: usize = 5000;
        const TEST_ITEMS: usize = 10000;
        
        let strategies = vec![
            ("DropOldest", OverflowStrategy::DropOldest),
            ("Backpressure", OverflowStrategy::Backpressure),
            ("Expand", OverflowStrategy::Expand),
        ];
        
        for (strategy_name, strategy) in strategies {
            println!("   Testing {} strategy", strategy_name);
            
            let buffer = AdaptiveBuffer::new(BUFFER_SIZE, MAX_SIZE, strategy.clone());
            let timer = PerformanceMeter::start();
            let memory_tracker = MemoryTracker::start();
            
            // Test high-throughput writes
            for i in 0..TEST_ITEMS {
                let data = vec![i as f32; 100]; // 100 samples each
                
                match buffer.push(data).await {
                    Ok(_) => {},
                    Err(_) => {
                        // Expected for Backpressure strategy when buffer is full
                        if matches!(strategy, OverflowStrategy::Backpressure) {
                            break;
                        }
                    }
                }
            }
            
            let processing_time = timer.elapsed_ms();
            let items_per_ms = TEST_ITEMS as f64 / processing_time as f64;
            
            // Test reads
            let read_timer = PerformanceMeter::start();
            let mut items_read = 0;
            
            while let Some(_item) = buffer.pop().await {
                items_read += 1;
            }
            
            let read_time = read_timer.elapsed_ms();
            
            println!("     Write: {:.1} items/ms, Read: {} items in {}ms", 
                    items_per_ms, items_read, read_time);
            
            // Memory check
            memory_tracker.check_memory_usage(&format!("{} buffer strategy", strategy_name), 50.0);
        }
        
        println!("✅ Buffer performance strategies tested");
        Ok(())
    }

    /// Test real-time processing capability
    #[tokio::test]
    async fn test_real_time_processing_capability() -> Result<()> {
        println!("🚀 Testing real-time processing capability");
        
        // Simulate real-time audio stream (16kHz, 100ms chunks)
        const SAMPLE_RATE: usize = 16000;
        const CHUNK_DURATION_MS: u32 = 100;
        const REAL_TIME_DURATION_S: u64 = 5; // 5 seconds of real-time simulation
        const CHUNK_INTERVAL_MS: u64 = 100; // New chunk every 100ms
        
        let generator = AudioTestGenerator::new(SAMPLE_RATE, CHUNK_DURATION_MS);
        let mut vad = DualChannelVad::new(SAMPLE_RATE)?;
        
        let start_time = Instant::now();
        let mut chunks_processed = 0;
        let mut total_processing_time = 0u64;
        let mut max_processing_time = 0u64;
        let mut processing_overruns = 0;
        
        println!("   Simulating {REAL_TIME_DURATION_S}s of real-time audio processing...");
        
        while start_time.elapsed().as_secs() < REAL_TIME_DURATION_S {
            let chunk_arrival_time = Instant::now();
            
            // Generate new audio chunk (simulating incoming audio)
            let audio_chunk = generator.generate_speech_pattern();
            
            // Process the chunk
            let process_timer = PerformanceMeter::start();
            let _result = vad.process_dual_channel(&audio_chunk, &[]).await?;
            let processing_time = process_timer.elapsed_ms();
            
            chunks_processed += 1;
            total_processing_time += processing_time;
            max_processing_time = max_processing_time.max(processing_time);
            
            // Check if processing exceeded real-time constraint
            if processing_time > CHUNK_INTERVAL_MS {
                processing_overruns += 1;
                println!("     Chunk {}: {}ms processing (overrun by {}ms)", 
                        chunks_processed, processing_time, processing_time - CHUNK_INTERVAL_MS);
            }
            
            // Wait for next chunk (simulating real-time audio arrival)
            let elapsed = chunk_arrival_time.elapsed();
            let target_interval = Duration::from_millis(CHUNK_INTERVAL_MS);
            
            if elapsed < target_interval {
                tokio::time::sleep(target_interval - elapsed).await;
            }
        }
        
        let total_duration = start_time.elapsed();
        let avg_processing_time = total_processing_time as f64 / chunks_processed as f64;
        let real_time_factor = total_duration.as_millis() as f64 / total_processing_time as f64;
        
        // Real-time performance validation
        assert!(processing_overruns == 0, 
               "Real-time processing failed: {} overruns out of {} chunks", 
               processing_overruns, chunks_processed);
        
        assert!(avg_processing_time < CHUNK_INTERVAL_MS as f64 * 0.8, 
               "Average processing time {:.1}ms exceeds 80% of real-time budget", 
               avg_processing_time);
        
        println!("✅ Real-time processing results:");
        println!("   Processed {} chunks in {:.1}s", chunks_processed, total_duration.as_secs_f64());
        println!("   Average processing: {:.1}ms per {}ms chunk", avg_processing_time, CHUNK_INTERVAL_MS);
        println!("   Max processing: {}ms", max_processing_time);
        println!("   Real-time factor: {:.1}x (>1.0 means faster than real-time)", real_time_factor);
        println!("   Processing overruns: {}", processing_overruns);
        
        Ok(())
    }

    /// Benchmark different VAD configurations
    #[tokio::test]
    async fn test_vad_configuration_benchmarks() -> Result<()> {
        println!("🚀 Benchmarking VAD configurations");
        
        let sample_rate = 16000;
        let generator = AudioTestGenerator::new(sample_rate, 2000);
        let test_audio = generator.generate_speech_with_pauses(800, 400); // Speech with pauses
        
        // Test different VAD configurations by creating fresh instances
        let configurations = vec![
            ("Standard", "Standard dual-channel VAD"),
            ("Reset", "VAD with reset between chunks"),
        ];
        
        let mut benchmark_results = Vec::new();
        
        for (config_name, description) in configurations {
            println!("   Testing {}: {}", config_name, description);
            
            let iterations = 10;
            let mut total_time = 0u64;
            let mut total_output_samples = 0;
            
            for i in 0..iterations {
                let mut vad = DualChannelVad::new(sample_rate)?;
                
                if config_name == "Reset" && i > 0 {
                    vad.reset(); // Test reset performance impact
                }
                
                let timer = PerformanceMeter::start();
                let result = vad.process_dual_channel(&test_audio, &[]).await?;
                let elapsed = timer.elapsed_ms();
                
                total_time += elapsed;
                total_output_samples += result.len();
            }
            
            let avg_time = total_time as f64 / iterations as f64;
            let avg_output = total_output_samples / iterations;
            let throughput = test_audio.len() as f64 / avg_time;
            
            benchmark_results.push((config_name, avg_time, avg_output, throughput));
            
            println!("     Avg time: {:.1}ms, Output samples: {}, Throughput: {:.0} samples/ms", 
                    avg_time, avg_output, throughput);
        }
        
        // Compare results
        println!("✅ VAD benchmark comparison:");
        benchmark_results.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap()); // Sort by throughput
        
        for (i, (name, time, output, throughput)) in benchmark_results.iter().enumerate() {
            let rank = match i {
                0 => "🥇 Best",
                1 => "🥈 Second", 
                _ => "🥉 Other",
            };
            println!("   {}: {} ({:.1}ms, {:.0} samples/ms)", rank, name, time, throughput);
        }
        
        Ok(())
    }
}

/// Run comprehensive performance test suite
#[cfg(test)]
pub async fn run_all_performance_tests() -> Result<()> {
    println!("⚡ Running comprehensive performance test suite...\n");

    println!("📊 Performance test categories:");
    println!("   - End-to-end latency requirements");
    println!("   - Sustained throughput under load");
    println!("   - Concurrent processing performance");
    println!("   - Memory usage patterns and leak detection");
    println!("   - Performance scaling across configurations");
    println!("   - Buffer performance strategies");
    println!("   - Real-time processing capability");
    println!("   - VAD configuration benchmarks");
    
    println!("\n✅ All performance tests defined and ready to run");
    println!("   Use 'cargo test performance_tests' to execute");

    Ok(())
}

/// Performance test utilities
pub struct PerformanceTestSuite;

impl PerformanceTestSuite {
    /// Run a single performance test with reporting
    pub async fn run_test<F, Fut>(
        test_name: &str,
        test_fn: F,
        max_duration_ms: u64,
    ) -> Result<u64>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        println!("🧪 Running {}", test_name);
        let timer = PerformanceMeter::start();
        
        test_fn().await?;
        
        let elapsed = timer.elapsed_ms();
        
        if elapsed <= max_duration_ms {
            println!("✅ {} completed in {}ms (within {}ms limit)", test_name, elapsed, max_duration_ms);
        } else {
            println!("❌ {} took {}ms (exceeded {}ms limit)", test_name, elapsed, max_duration_ms);
        }
        
        Ok(elapsed)
    }

    /// Generate performance report
    pub fn generate_report(test_results: Vec<(&str, u64, u64)>) {
        println!("\n📈 Performance Test Summary Report");
        println!("=" .repeat(50));
        
        let mut total_time = 0;
        let mut passed = 0;
        let mut failed = 0;
        
        for (test_name, elapsed_ms, limit_ms) in test_results {
            total_time += elapsed_ms;
            
            let status = if elapsed_ms <= limit_ms {
                passed += 1;
                "PASS"
            } else {
                failed += 1;
                "FAIL"
            };
            
            let percentage = (elapsed_ms as f64 / limit_ms as f64) * 100.0;
            
            println!("{:30} {:>6} {:>8}ms / {:>6}ms ({:>5.1}%)", 
                    test_name, status, elapsed_ms, limit_ms, percentage);
        }
        
        println!("-".repeat(50));
        println!("Total Tests: {} | Passed: {} | Failed: {} | Total Time: {}ms", 
                passed + failed, passed, failed, total_time);
        
        if failed == 0 {
            println!("🎉 All performance tests PASSED!");
        } else {
            println!("⚠️  {} performance test(s) FAILED", failed);
        }
    }
}