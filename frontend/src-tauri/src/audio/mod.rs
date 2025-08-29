// src/audio/mod.rs
pub mod core;
pub mod audio_processing;
pub mod encode;
pub mod ffmpeg;
pub mod vad;
pub mod streaming_vad;
pub mod buffer;
pub mod channel;
pub mod error;
pub mod intelligent_chunking;
pub mod streaming_whisper;
pub mod context_manager;

#[cfg(test)]
pub mod tests;

pub use core::{
    default_input_device, default_output_device, get_device_and_config, list_audio_devices,
    parse_audio_device, trigger_audio_permission,
    AudioDevice, AudioStream, AudioTranscriptionEngine, DeviceControl, DeviceType,
    LAST_AUDIO_CAPTURE,
};
pub use encode::{
    encode_single_audio, AudioInput
};
pub use vad::{
    extract_speech_16k, DualChannelVad, DualChannelVadStats
};
pub use streaming_vad::{
    StreamingVadProcessor, StreamingVadConfig, StreamingResult, 
    BoundaryInfo, SpeechBoundaryDetector, VadStatistics
};
pub use buffer::{
    AdaptiveBuffer, BufferMetrics, OverflowStrategy
};
pub use channel::{
    ManagedChannel, ChannelState, RecoveryStrategy, HealthMonitor, ChannelHealthMetrics
};
pub use error::{
    AudioError, ErrorHandler, ErrorRecoveryAction, ErrorRecoveryStrategy,
    ErrorContext, ErrorStatistics, create_error_context
};
pub use intelligent_chunking::{
    IntelligentChunker, ChunkingConfig, ChunkedAudio, BoundaryType, ContextBuffer
};
pub use streaming_whisper::{
    StreamingWhisperService, StreamingWhisperConfig, StreamingTranscriptionResult, 
    TranscriptionSegment, StreamingStats
};
pub use context_manager::{
    StreamingTranscriptionContextManager, ContextManagerConfig, ContextManagerEvent,
    EnhancedTranscriptionResult, ContextManagerStatus, ProcessingStats, AudioSourceConfig
};