//! Comprehensive test suite for the streaming transcription system
//! 
//! This module contains unit tests, integration tests, performance tests,
//! and stress tests for all components of the streaming audio transcription pipeline.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use anyhow::Result;

/// Test utilities and helpers
pub mod test_utils;

/// Tests for adaptive buffer management
pub mod buffer_tests;

/// Tests for managed channels and recovery
pub mod channel_tests;

/// Tests for error handling and recovery
pub mod error_tests;

/// Tests for streaming VAD processor
pub mod vad_tests;

/// Tests for intelligent chunking
pub mod chunking_tests;

/// Tests for streaming whisper service
pub mod whisper_tests;

/// Integration tests for the complete pipeline
pub mod integration_tests;

/// Performance and stress tests
pub mod performance_tests;

/// End-to-end tests with real audio data
pub mod e2e_tests;

// Re-export all test utilities
pub use test_utils::*;