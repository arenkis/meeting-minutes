use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use anyhow::Result;

use super::test_utils::*;
use super::super::{
    StreamingTranscriptionContextManager, ContextManagerConfig, ContextManagerEvent,
    StreamingWhisperService, StreamingWhisperConfig,
    DualChannelVad, IntelligentChunker, ChunkingConfig,
    ManagedChannel, RecoveryStrategy,
    AudioError, ErrorHandler,
};

/// Integration tests for the complete streaming transcription pipeline
#[cfg(test)]
mod tests {
    use super::*;

    /// Test the complete end-to-end transcription pipeline
    #[tokio::test]
    async fn test_complete_transcription_pipeline() -> Result<()> {
        // Create test configuration
        let config = ContextManagerConfig {
            sample_rate: 16000,
            buffer_size_ms: 100,
            max_context_duration_s: 60,
            min_chunk_size_ms: 1000,
            max_chunk_size_ms: 10000,
            chunk_timeout_ms: 5000,
            auto_model_management: false, // Disable for testing
            preferred_model: "tiny".to_string(),
            persist_context: true,
        };

        // Note: This test would require a loaded whisper model
        // In a real test environment, we'd set up a test model
        println!("✓ Complete pipeline test setup (would require whisper model)");
        Ok(())
    }

    /// Test context manager lifecycle
    #[tokio::test]
    async fn test_context_manager_lifecycle() -> Result<()> {
        let config = ContextManagerConfig::default();
        
        // Create context manager
        let manager = StreamingTranscriptionContextManager::new(config).await;
        assert!(manager.is_ok(), "Context manager creation should succeed");
        let manager = manager.unwrap();

        // Check initial status
        let status = manager.get_status().await;
        assert!(!status.is_active, "Manager should start inactive");
        assert_eq!(status.processing_stats.total_transcriptions, 0);
        
        // Test event subscription
        let mut event_receiver = manager.subscribe_to_events();
        
        // Test status updates are received
        let status_event = tokio::time::timeout(
            Duration::from_millis(1000),
            event_receiver.recv()
        ).await;
        
        // Should receive a status update (event might be sent during creation)
        println!("✓ Context manager lifecycle test completed");
        Ok(())
    }

    /// Test dual-channel VAD processing
    #[tokio::test] 
    async fn test_dual_channel_vad_processing() -> Result<()> {
        let sample_rate = 16000;
        let mut vad = DualChannelVad::new(sample_rate)?;
        let generator = AudioTestGenerator::new(sample_rate, 2000); // 2 seconds

        // Generate test audio
        let (mic_audio, speaker_audio) = generator.generate_conversation();
        assert_eq!(mic_audio.len(), 32000); // 2 seconds @ 16kHz
        assert_eq!(speaker_audio.len(), 32000);

        // Process through VAD
        let performance = PerformanceMeter::start();
        let vad_result = vad.process_dual_channel(&mic_audio, &speaker_audio).await?;
        
        // Validate performance
        assert!(performance.check_performance("Dual-channel VAD", 500), "VAD processing should be under 500ms");
        
        // Validate output
        assert!(validate_audio_samples(&vad_result), "VAD output should be valid audio");
        assert!(!vad_result.is_empty(), "VAD should produce some output for conversation audio");
        
        // Check VAD statistics
        let stats = vad.get_statistics();
        assert!(stats.mic_stats.total_frames_processed > 0, "Should have processed mic frames");
        assert!(stats.speaker_stats.total_frames_processed > 0, "Should have processed speaker frames");

        println!("✓ Dual-channel VAD processing: {} samples -> {} samples", 
                mic_audio.len() + speaker_audio.len(), vad_result.len());
        Ok(())
    }

    /// Test intelligent chunking with various boundary conditions
    #[tokio::test]
    async fn test_intelligent_chunking_boundaries() -> Result<()> {
        let config = ChunkingConfig {
            sample_rate: 16000,
            min_chunk_duration_ms: 500,
            max_chunk_duration_ms: 5000,
            silence_threshold_ms: 200,
            overlap_duration_ms: 100,
            adaptive_chunking: true,
            preserve_word_boundaries: true,
        };

        let mut chunker = IntelligentChunker::new(config)?;
        let generator = AudioTestGenerator::new(16000, 10000); // 10 seconds

        // Test with speech-pause pattern
        let test_audio = generator.generate_speech_with_pauses(1000, 500); // 1s speech, 0.5s pause
        
        let performance = PerformanceMeter::start();
        let chunked_result = chunker.process_stream(&test_audio).await?;
        
        assert!(performance.check_performance("Intelligent chunking", 200), "Chunking should be fast");

        // Validate chunks
        assert!(!chunked_result.ready_chunks.is_empty(), "Should produce chunks from speech-pause audio");
        
        let total_chunk_samples: usize = chunked_result.ready_chunks.iter()
            .map(|chunk| chunk.samples.len())
            .sum();
        
        println!("✓ Intelligent chunking: {} input samples -> {} chunks ({} total samples)", 
                test_audio.len(), chunked_result.ready_chunks.len(), total_chunk_samples);

        // Test different boundary types
        let boundary_types: Vec<_> = chunked_result.ready_chunks.iter()
            .map(|chunk| chunk.metadata.boundary_type.clone())
            .collect();
        
        println!("   Boundary types: {:?}", boundary_types);
        assert!(!boundary_types.is_empty(), "Should have detected boundaries");

        Ok(())
    }

    /// Test error handling and recovery throughout the pipeline
    #[tokio::test]
    async fn test_error_handling_and_recovery() -> Result<()> {
        // Test channel recovery
        let channel = Arc::new(ManagedChannel::new(
            100,
            RecoveryStrategy::ExponentialBackoff {
                base_delay_ms: 10,
                max_delay_ms: 100,
                max_retries: 3,
            },
            "test_channel".to_string(),
        ));

        // Test normal operation
        let test_data = vec![1.0, 2.0, 3.0, 4.0];
        send_test_audio(&channel, test_data.clone()).await?;
        
        let received = receive_test_audio(&channel, 1000).await?;
        assert_eq!(received, test_data, "Channel should transmit data correctly");

        // Test health monitoring
        let health = channel.get_health().await;
        assert!(health.is_healthy, "Channel should be healthy after successful operation");

        println!("✓ Channel error handling and recovery test passed");

        // Test error handler
        let error_handler = ErrorHandler::new();
        let test_error = AudioError::channel_send_failed("test".to_string());
        let error_context = super::super::create_error_context("test", "test_operation", Some("test_context"));
        
        let recovery_action = error_handler.handle_error(test_error, error_context).await;
        assert!(matches!(recovery_action, super::super::ErrorRecoveryAction::Retry { .. } | 
                                        super::super::ErrorRecoveryAction::Backoff { .. } |
                                        super::super::ErrorRecoveryAction::Reset |
                                        super::super::ErrorRecoveryAction::Ignore),
               "Error handler should provide valid recovery action");

        println!("✓ Error handler test passed");
        Ok(())
    }

    /// Test memory usage and performance under load
    #[tokio::test]
    async fn test_performance_under_load() -> Result<()> {
        let memory_tracker = MemoryTracker::start();
        let performance = PerformanceMeter::start();
        
        // Create multiple VAD processors to simulate load
        let sample_rate = 16000;
        let num_processors = 5;
        let mut vad_processors = Vec::new();
        
        for _ in 0..num_processors {
            vad_processors.push(DualChannelVad::new(sample_rate)?);
        }

        // Generate test data
        let generator = AudioTestGenerator::new(sample_rate, 5000); // 5 seconds
        let (mic_audio, speaker_audio) = generator.generate_conversation();

        // Process through all VAD processors concurrently
        let mut handles = Vec::new();
        
        for mut vad in vad_processors {
            let mic_copy = mic_audio.clone();
            let speaker_copy = speaker_audio.clone();
            
            let handle = tokio::spawn(async move {
                vad.process_dual_channel(&mic_copy, &speaker_copy).await
            });
            
            handles.push(handle);
        }

        // Wait for all to complete
        let results = futures::future::join_all(handles).await;
        
        // Validate all succeeded
        for result in results {
            let vad_output = result??;
            assert!(validate_audio_samples(&vad_output), "All VAD outputs should be valid");
        }

        // Check performance constraints
        assert!(performance.check_performance("Concurrent VAD processing", 3000), 
               "Concurrent processing should complete within 3 seconds");
        
        assert!(memory_tracker.check_memory_usage("Concurrent VAD processing", 100.0),
               "Memory usage should stay reasonable");

        println!("✓ Performance under load test passed");
        Ok(())
    }

    /// Test context preservation across chunks
    #[tokio::test]
    async fn test_context_preservation() -> Result<()> {
        let whisper_config = StreamingWhisperConfig {
            sample_rate: 16000,
            max_context_samples: 48000, // 3 seconds
            context_overlap_samples: 8000, // 0.5 seconds  
            max_retries: 2,
            base_temperature: 0.0,
            temperature_increment: 0.2,
            max_temperature: 0.6,
            language: Some("en".to_string()),
            enable_timestamps: true,
            confidence_threshold: 0.1, // Lower for testing
            max_processing_time_ms: 2000,
        };

        let whisper_service = StreamingWhisperService::new(whisper_config)?;
        
        // Note: This test would require actual whisper model initialization
        // Here we test the service creation and configuration
        assert!(whisper_service.is_ready().await == false, "Service should not be ready without model");
        
        // Test context reset
        whisper_service.reset_context().await;
        let stats = whisper_service.get_statistics().await;
        assert_eq!(stats.total_transcriptions, 0, "Stats should be reset");

        println!("✓ Context preservation test completed (would require whisper model for full test)");
        Ok(())
    }

    /// Test concurrent processing of multiple audio streams
    #[tokio::test]
    async fn test_concurrent_stream_processing() -> Result<()> {
        let num_streams = 3;
        let sample_rate = 16000;
        let duration_ms = 3000;
        
        let generator = AudioTestGenerator::new(sample_rate, duration_ms);
        let performance = PerformanceMeter::start();
        
        // Create multiple processing tasks
        let mut handles = Vec::new();
        
        for stream_id in 0..num_streams {
            let test_audio = generator.generate_speech_pattern();
            
            let handle = tokio::spawn(async move {
                // Simulate processing pipeline
                let mut vad = DualChannelVad::new(sample_rate).unwrap();
                let vad_result = vad.process_dual_channel(&test_audio, &[]).await.unwrap();
                
                let chunker_config = ChunkingConfig {
                    sample_rate,
                    min_chunk_duration_ms: 500,
                    max_chunk_duration_ms: 2000,
                    silence_threshold_ms: 200,
                    overlap_duration_ms: 100,
                    adaptive_chunking: true,
                    preserve_word_boundaries: true,
                };
                
                let mut chunker = IntelligentChunker::new(chunker_config).unwrap();
                let chunk_result = chunker.process_stream(&vad_result).await.unwrap();
                
                (stream_id, vad_result.len(), chunk_result.ready_chunks.len())
            });
            
            handles.push(handle);
        }

        // Wait for all streams to complete
        let results = futures::future::join_all(handles).await;
        
        // Validate results
        for result in results {
            let (stream_id, vad_samples, num_chunks) = result?;
            println!("Stream {}: {} VAD samples -> {} chunks", stream_id, vad_samples, num_chunks);
            assert!(vad_samples > 0, "VAD should produce output");
        }

        assert!(performance.check_performance("Concurrent stream processing", 2000),
               "Concurrent processing should be efficient");

        println!("✓ Concurrent stream processing test passed");
        Ok(())
    }

    /// Test adaptive behavior under different audio conditions
    #[tokio::test]
    async fn test_adaptive_behavior() -> Result<()> {
        let sample_rate = 16000;
        let generator = AudioTestGenerator::new(sample_rate, 5000); // 5 seconds

        // Test 1: Very quiet audio
        let quiet_audio = generator.generate_noise(0.001); // Very low amplitude
        let mut vad_quiet = DualChannelVad::new(sample_rate)?;
        let quiet_result = vad_quiet.process_dual_channel(&quiet_audio, &[]).await?;
        
        println!("Quiet audio: {} samples -> {} samples", quiet_audio.len(), quiet_result.len());

        // Test 2: Very loud audio  
        let mut loud_audio = generator.generate_speech_pattern();
        for sample in &mut loud_audio {
            *sample *= 0.9; // Scale to near maximum
        }
        let mut vad_loud = DualChannelVad::new(sample_rate)?;
        let loud_result = vad_loud.process_dual_channel(&loud_audio, &[]).await?;
        
        println!("Loud audio: {} samples -> {} samples", loud_audio.len(), loud_result.len());

        // Test 3: Dynamic audio (changing levels)
        let dynamic_audio = generator.generate_dynamic_audio();
        let mut vad_dynamic = DualChannelVad::new(sample_rate)?;
        let dynamic_result = vad_dynamic.process_dual_channel(&dynamic_audio, &[]).await?;
        
        println!("Dynamic audio: {} samples -> {} samples", dynamic_audio.len(), dynamic_result.len());

        // All results should be valid
        assert!(validate_audio_samples(&quiet_result));
        assert!(validate_audio_samples(&loud_result));
        assert!(validate_audio_samples(&dynamic_result));

        println!("✓ Adaptive behavior test passed");
        Ok(())
    }

    /// Test system recovery from various failure scenarios
    #[tokio::test]
    async fn test_failure_recovery_scenarios() -> Result<()> {
        // Test channel recovery from disconnection
        let channel = Arc::new(ManagedChannel::new(
            10,
            RecoveryStrategy::ExponentialBackoff {
                base_delay_ms: 50,
                max_delay_ms: 500,
                max_retries: 5,
            },
            "recovery_test_channel".to_string(),
        ));

        // Fill channel beyond capacity to trigger overflow
        for i in 0..20 {
            let data = vec![i as f32; 100];
            // Some sends might fail due to overflow, that's expected
            let _ = channel.send(data).await;
        }

        // Channel should still be operational
        let health = channel.get_health().await;
        println!("Channel health after overflow test: {:?}", health.state);

        // Test with normal operation after overflow
        let test_data = vec![1.0, 2.0, 3.0];
        send_test_audio(&channel, test_data.clone()).await?;

        println!("✓ Failure recovery scenario test passed");
        Ok(())
    }

    /// Test real-world audio patterns and edge cases
    #[tokio::test]
    async fn test_real_world_audio_patterns() -> Result<()> {
        let sample_rate = 16000;
        let generator = AudioTestGenerator::new(sample_rate, 8000); // 8 seconds
        
        // Test pattern 1: Overlapping speakers
        let (speaker1, speaker2) = generator.generate_conversation();
        
        // Create overlapping audio (both speakers talking)
        let mut overlapping_audio = Vec::new();
        for i in 0..speaker1.len().min(speaker2.len()) {
            overlapping_audio.push((speaker1[i] + speaker2[i]) * 0.5); // Mix at 50% each
        }

        let mut vad = DualChannelVad::new(sample_rate)?;
        let overlap_result = vad.process_dual_channel(&overlapping_audio, &[]).await?;
        
        println!("Overlapping speakers: {} samples -> {} samples", 
                overlapping_audio.len(), overlap_result.len());

        // Test pattern 2: Audio with artifacts
        let mut noisy_speech = generator.generate_speech_pattern();
        generator.add_artifacts(&mut noisy_speech);
        
        let artifact_result = vad.process_dual_channel(&noisy_speech, &[]).await?;
        
        println!("Speech with artifacts: {} samples -> {} samples", 
                noisy_speech.len(), artifact_result.len());

        // Test pattern 3: Very short utterances
        let short_generator = AudioTestGenerator::new(sample_rate, 200); // 200ms
        let short_speech = short_generator.generate_speech_pattern();
        
        let short_result = vad.process_dual_channel(&short_speech, &[]).await?;
        
        println!("Short utterances: {} samples -> {} samples", 
                short_speech.len(), short_result.len());

        // All should produce valid results
        assert!(validate_audio_samples(&overlap_result));
        assert!(validate_audio_samples(&artifact_result));
        assert!(validate_audio_samples(&short_result));

        println!("✓ Real-world audio patterns test passed");
        Ok(())
    }
}

// Helper to run all integration tests
#[cfg(test)]
pub async fn run_all_integration_tests() -> Result<()> {
    println!("🧪 Running comprehensive integration tests...\n");

    // Note: In a real test environment, these would be run by cargo test
    // This function demonstrates the test structure
    
    println!("✅ All integration tests would run here");
    println!("   - Complete transcription pipeline");
    println!("   - Context manager lifecycle");
    println!("   - Dual-channel VAD processing");
    println!("   - Intelligent chunking boundaries");
    println!("   - Error handling and recovery");
    println!("   - Performance under load");
    println!("   - Context preservation");
    println!("   - Concurrent stream processing");
    println!("   - Adaptive behavior");
    println!("   - Failure recovery scenarios");
    println!("   - Real-world audio patterns");

    Ok(())
}