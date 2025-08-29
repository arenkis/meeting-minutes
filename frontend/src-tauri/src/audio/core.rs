use super::audio_processing::audio_to_mono;
use super::channel::{ManagedChannel, RecoveryStrategy};
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::StreamError;
use lazy_static::lazy_static;
use log::{ error, info, warn, debug};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use std::{fmt, thread};
use tokio::sync::{broadcast, oneshot};
lazy_static! {
    pub static ref LAST_AUDIO_CAPTURE: AtomicU64 = AtomicU64::new(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );
}

#[derive(Clone, Debug, PartialEq)]
pub enum AudioTranscriptionEngine {
    Deepgram,
    WhisperTiny,
    WhisperDistilLargeV3,
    WhisperLargeV3Turbo,
    WhisperLargeV3,
}

impl fmt::Display for AudioTranscriptionEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioTranscriptionEngine::Deepgram => write!(f, "Deepgram"),
            AudioTranscriptionEngine::WhisperTiny => write!(f, "WhisperTiny"),
            AudioTranscriptionEngine::WhisperDistilLargeV3 => write!(f, "WhisperLarge"),
            AudioTranscriptionEngine::WhisperLargeV3Turbo => write!(f, "WhisperLargeV3Turbo"),
            AudioTranscriptionEngine::WhisperLargeV3 => write!(f, "WhisperLargeV3"),
        }
    }
}

impl Default for AudioTranscriptionEngine {
    fn default() -> Self {
        AudioTranscriptionEngine::WhisperLargeV3Turbo
    }
}

#[derive(Clone, Debug)]
pub struct DeviceControl {
    pub is_running: bool,
    pub is_paused: bool,
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Debug, Deserialize)]
pub enum DeviceType {
    Input,
    Output,
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Debug)]
pub struct AudioDevice {
    pub name: String,
    pub device_type: DeviceType,
}

impl AudioDevice {
    pub fn new(name: String, device_type: DeviceType) -> Self {
        AudioDevice { name, device_type }
    }

    pub fn from_name(name: &str) -> Result<Self> {
        if name.trim().is_empty() {
            return Err(anyhow!("Device name cannot be empty"));
        }

        let (name, device_type) = if name.to_lowercase().ends_with("(input)") {
            (
                name.trim_end_matches("(input)").trim().to_string(),
                DeviceType::Input,
            )
        } else if name.to_lowercase().ends_with("(output)") {
            (
                name.trim_end_matches("(output)").trim().to_string(),
                DeviceType::Output,
            )
        } else {
            return Err(anyhow!(
                "Device type (input/output) not specified in the name"
            ));
        };

        Ok(AudioDevice::new(name, device_type))
    }
}

impl fmt::Display for AudioDevice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} ({})",
            self.name,
            match self.device_type {
                DeviceType::Input => "input",
                DeviceType::Output => "output",
            }
        )
    }
}

pub fn parse_audio_device(name: &str) -> Result<AudioDevice> {
    AudioDevice::from_name(name)
}

// Platform-specific audio device configurations
#[cfg(target_os = "windows")]
fn configure_windows_audio(host: &cpal::Host) -> Result<Vec<AudioDevice>> {
    let mut devices = Vec::new();
    
    // Get WASAPI devices
    if let Ok(wasapi_host) = cpal::host_from_id(cpal::HostId::Wasapi) {
        info!("Using WASAPI host for Windows audio device enumeration");
        
        // Add output devices (including loopback)
        if let Ok(output_devices) = wasapi_host.output_devices() {
            for device in output_devices {
                if let Ok(name) = device.name() {
                    // For Windows, we need to mark output devices specifically for loopback
                    info!("Found Windows output device: {}", name);
                    devices.push(AudioDevice::new(name.clone(), DeviceType::Output));
                }
            }
        } else {
            warn!("Failed to enumerate WASAPI output devices");
        }

        // Add input devices from WASAPI
        if let Ok(input_devices) = wasapi_host.input_devices() {
            for device in input_devices {
                if let Ok(name) = device.name() {
                    info!("Found Windows input device: {}", name);
                    devices.push(AudioDevice::new(name.clone(), DeviceType::Input));
                }
            }
        } else {
            warn!("Failed to enumerate WASAPI input devices");
        }
    } else {
        warn!("Failed to create WASAPI host, falling back to default host");
    }
    
    // If WASAPI failed or returned no devices, try default host as fallback
    if devices.is_empty() {
        debug!("WASAPI device enumeration failed or returned no devices, falling back to default host");
        // Add regular input devices
        if let Ok(input_devices) = host.input_devices() {
            for device in input_devices {
                if let Ok(name) = device.name() {
                    info!("Found fallback input device: {}", name);
                    devices.push(AudioDevice::new(name.clone(), DeviceType::Input));
                }
            }
        } else {
            warn!("Failed to enumerate input devices from default host");
        }

        // Add output devices
        if let Ok(output_devices) = host.output_devices() {
            for device in output_devices {
                if let Ok(name) = device.name() {
                    info!("Found fallback output device: {}", name);
                    devices.push(AudioDevice::new(name.clone(), DeviceType::Output));
                }
            }
        } else {
            warn!("Failed to enumerate output devices from default host");
        }
    }
    
    // If we still have no devices, add default devices
    if devices.is_empty() {
        warn!("No audio devices found, adding default devices only");
        
        // Try to add default input device
        if let Some(device) = host.default_input_device() {
            if let Ok(name) = device.name() {
                info!("Adding default input device: {}", name);
                devices.push(AudioDevice::new(name, DeviceType::Input));
            }
        }
        
        // Try to add default output device
        if let Some(device) = host.default_output_device() {
            if let Ok(name) = device.name() {
                info!("Adding default output device: {}", name);
                devices.push(AudioDevice::new(name, DeviceType::Output));
            }
        }
    }
    
    info!("Found {} Windows audio devices", devices.len());
    Ok(devices)
}

#[cfg(target_os = "linux")]
fn configure_linux_audio(host: &cpal::Host) -> Result<Vec<AudioDevice>> {
    let mut devices = Vec::new();
    
    // Add input devices
    for device in host.input_devices()? {
        if let Ok(name) = device.name() {
            devices.push(AudioDevice::new(name, DeviceType::Input));
        }
    }
    
    // Add PulseAudio monitor sources for system audio
    if let Ok(pulse_host) = cpal::host_from_id(cpal::HostId::Pulse) {
        for device in pulse_host.input_devices()? {
            if let Ok(name) = device.name() {
                // Check if it's a monitor source
                if name.contains("monitor") {
                    devices.push(AudioDevice::new(
                        format!("{} (System Audio)", name),
                        DeviceType::Output
                    ));
                }
            }
        }
    }
    
    Ok(devices)
}

pub async fn list_audio_devices() -> Result<Vec<AudioDevice>> {
    let host = cpal::default_host();
    let mut devices = Vec::new();

    // Platform-specific device enumeration
    #[cfg(target_os = "windows")]
    {
        devices = configure_windows_audio(&host)?;
    }

    #[cfg(target_os = "linux")]
    {
        devices = configure_linux_audio(&host)?;
    }

    #[cfg(target_os = "macos")]
    {
        // Existing macOS implementation
        for device in host.input_devices()? {
            if let Ok(name) = device.name() {
                devices.push(AudioDevice::new(name, DeviceType::Input));
            }
        }

        // Filter function to exclude macOS speakers and AirPods for output devices
        fn should_include_output_device(name: &str) -> bool {
            !name.to_lowercase().contains("speakers") && !name.to_lowercase().contains("airpods")
        }

        if let Ok(host) = cpal::host_from_id(cpal::HostId::ScreenCaptureKit) {
            for device in host.input_devices()? {
                if let Ok(name) = device.name() {
                    if should_include_output_device(&name) {
                        devices.push(AudioDevice::new(name, DeviceType::Output));
                    }
                }
            }
        }

        for device in host.output_devices()? {
            if let Ok(name) = device.name() {
                if should_include_output_device(&name) {
                    devices.push(AudioDevice::new(name, DeviceType::Output));
                }
            }
        }
    }

    // Add any additional devices from the default host
    if let Ok(other_devices) = host.devices() {
        for device in other_devices {
            if let Ok(name) = device.name() {
                if !devices.iter().any(|d| d.name == name) {
                    devices.push(AudioDevice::new(name, DeviceType::Output));
                }
            }
        }
    }

    Ok(devices)
}

pub fn default_input_device() -> Result<AudioDevice> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("No default input device found"))?;
    Ok(AudioDevice::new(device.name()?, DeviceType::Input))
}

pub fn default_output_device() -> Result<AudioDevice> {
    #[cfg(target_os = "macos")]
    {
        // ! see https://github.com/RustAudio/cpal/pull/894
        if let Ok(host) = cpal::host_from_id(cpal::HostId::ScreenCaptureKit) {
            if let Some(device) = host.default_input_device() {
                if let Ok(name) = device.name() {
                    return Ok(AudioDevice::new(name, DeviceType::Output));
                }
            }
        }
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("No default output device found"))?;
        return Ok(AudioDevice::new(device.name()?, DeviceType::Output));
    }

    #[cfg(target_os = "windows")]
    {
        // Try WASAPI host first for Windows
        if let Ok(wasapi_host) = cpal::host_from_id(cpal::HostId::Wasapi) {
            if let Some(device) = wasapi_host.default_output_device() {
                if let Ok(name) = device.name() {
                    return Ok(AudioDevice::new(name, DeviceType::Output));
                }
            }
        }
        // Fallback to default host if WASAPI fails
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("No default output device found"))?;
        return Ok(AudioDevice::new(device.name()?, DeviceType::Output));
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("No default output device found"))?;
        return Ok(AudioDevice::new(device.name()?, DeviceType::Output));
    }
}

pub fn trigger_audio_permission() -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("No default input device found"))?;

    let config = device.default_input_config()?;

    // Build and start an input stream to trigger the permission request
    let stream = device.build_input_stream(
        &config.into(),
        |_data: &[f32], _: &cpal::InputCallbackInfo| {
            // Do nothing, we just want to trigger the permission request
        },
        |err| error!("Error in audio stream: {}", err),
        None,
    )?;

    // Start the stream to actually trigger the permission dialog
    stream.play()?;

    // Sleep briefly to allow the permission dialog to appear
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Stop the stream
    drop(stream);

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn request_screen_recording_permission() -> Result<()> {
    use std::process::Command;
    
    info!("Requesting Screen Recording permission for system audio capture");
    
    // Check if we already have permission
    let output = Command::new("sh")
        .arg("-c")
        .arg("sqlite3 ~/Library/Application\\ Support/com.apple.TCC/TCC.db \"SELECT allowed FROM access WHERE service='kTCCServiceScreenCapture' AND client LIKE '%meetily%'\" 2>/dev/null || echo '0'")
        .output();
    
    match output {
        Ok(output) => {
            let result_str = String::from_utf8_lossy(&output.stdout);
            let result = result_str.trim();
            if result == "1" {
                info!("Screen Recording permission already granted");
                return Ok(());
            }
        }
        Err(e) => {
            warn!("Could not check Screen Recording permission status: {}", e);
        }
    }
    
    // Try to programmatically request permission by accessing screen capture
    match Command::new("osascript")
        .arg("-e")
        .arg(r#"
        tell application "System Events"
            try
                set frontApp to first application process whose frontmost is true
                return "success"
            on error
                return "denied"
            end try
        end tell
        "#)
        .output()
    {
        Ok(output) => {
            let result_str = String::from_utf8_lossy(&output.stdout);
            let result = result_str.trim();
            if result.contains("denied") {
                warn!("Screen Recording permission required but not granted");
                // Open System Preferences to the correct pane
                let _ = Command::new("open")
                    .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
                    .spawn();
                return Err(anyhow!("Screen Recording permission required. Please enable it in System Settings → Privacy & Security → Screen Recording"));
            }
        }
        Err(e) => {
            warn!("Failed to check screen recording access: {}", e);
        }
    }
    
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn request_screen_recording_permission() -> Result<()> {
    // Not needed on other platforms
    Ok(())
}

#[derive(Clone)]
pub struct AudioStream {
    pub device: Arc<AudioDevice>,
    pub device_config: cpal::SupportedStreamConfig,
    managed_channel: Arc<ManagedChannel<Vec<f32>>>,
    broadcast_sender: broadcast::Sender<Vec<f32>>,
    stream_control: mpsc::Sender<StreamControl>,
    stream_thread: Option<Arc<tokio::sync::Mutex<Option<thread::JoinHandle<()>>>>>,
    is_disconnected: Arc<AtomicBool>,
}

enum StreamControl {
    Stop(oneshot::Sender<()>),
    Recover(oneshot::Sender<()>),
}

impl AudioStream {
    pub async fn from_device(
        device: Arc<AudioDevice>,
        is_running: Arc<AtomicBool>,
    ) -> Result<Self> {
        info!("Initializing audio stream for device: {}", device.to_string());
        
        // Create managed channel with recovery strategy
        let channel_id = format!("audio_stream_{}", device.to_string());
        let managed_channel = Arc::new(
            ManagedChannel::new(
                1000, // Initial capacity
                RecoveryStrategy::ExponentialBackoff {
                    base_delay_ms: 100,
                    max_delay_ms: 5000,
                    max_retries: 5,
                },
                channel_id,
            )
        );
        
        // Get device and config with improved error handling
        let (cpal_audio_device, config) = match get_device_and_config(&device).await {
            Ok((device, config)) => {
                info!("Successfully got device and config for: {}", device.name()?);
                (device, config)
            },
            Err(e) => {
                error!("Failed to get device and config: {}", e);
                return Err(anyhow!("Failed to initialize audio device: {}", e));
            }
        };
        
        // Verify we can actually get input config for input devices
        if device.device_type == DeviceType::Input {
            match cpal_audio_device.default_input_config() {
                Ok(conf) => info!("Default input config: {:?}", conf),
                Err(e) => {
                    error!("Failed to get default input config: {}", e);
                    
                    // On Windows, we might still be able to use the device with our custom config
                    #[cfg(not(target_os = "windows"))]
                    return Err(anyhow!("Failed to get default input config: {}", e));
                    
                    #[cfg(target_os = "windows")]
                    {
                        warn!("Continuing with custom config despite default config error on Windows");
                        // Try to verify we can at least get supported configs
                        match cpal_audio_device.supported_input_configs() {
                            Ok(configs) => {
                                let count = configs.count();
                                if count == 0 {
                                    error!("No supported input configurations available for this device");
                                    return Err(anyhow!("No supported input configurations available for device: {}", device.name));
                                }
                                info!("Device has {} supported input configurations", count);
                            },
                            Err(e) => {
                                error!("Failed to get supported input configs: {}", e);
                                // Still continue as our custom config might work
                            }
                        }
                    }
                }

            }
        }
        
        let channels = config.channels();
        info!("Audio config - Sample rate: {}, Channels: {}, Format: {:?}", 
            config.sample_rate().0, channels, config.sample_format());

        // Create a direct broadcast channel for sync operations from audio callback
        let (broadcast_sender, _) = broadcast::channel::<Vec<f32>>(1000);

        let is_running_weak_2 = Arc::downgrade(&is_running);
        let is_disconnected = Arc::new(AtomicBool::new(false));
        let device_clone = device.clone();
        let config_clone = config.clone();
        let managed_channel_clone = managed_channel.clone();
        let broadcast_sender_clone = broadcast_sender.clone();
        let (stream_control_tx, stream_control_rx) = mpsc::channel();

        let is_disconnected_clone = is_disconnected.clone();
        let stream_control_tx_clone = stream_control_tx.clone();
        let stream_thread = Arc::new(tokio::sync::Mutex::new(Some(thread::spawn(move || {
            let device = device_clone;
            let device_name = device.to_string();
            let device_name_clone = device_name.clone();  // Clone for the closure
            let config = config_clone;
            let managed_channel = managed_channel_clone;
            info!("Starting audio stream thread for device: {}", device_name);
            let is_running_weak_for_error = is_running_weak_2.clone();
            let is_running_weak_for_data = is_running_weak_2.clone();
            let error_callback = move |err: StreamError| {
                let error_msg = err.to_string();
                let error_lower = error_msg.to_lowercase();
                
                // 🔄 Improved Error Recovery Logic
                if error_msg.contains("The requested device is no longer available") ||
                   error_msg.contains("device is no longer valid") {
                    warn!(
                        "🔄 Audio device {} temporarily unavailable, attempting recovery...",
                        device_name_clone
                    );
                    
                    // Instead of immediately stopping, mark as disconnected and let the main loop handle reconnection
                    is_disconnected_clone.store(true, Ordering::Relaxed);
                    
                    // Send a recovery signal instead of stop
                    if let Err(e) = stream_control_tx_clone.send(StreamControl::Recover(oneshot::channel().0)) {
                        warn!("Failed to send recovery signal: {}", e);
                        // Fallback to stop if recovery signal fails
                        let _ = stream_control_tx_clone.send(StreamControl::Stop(oneshot::channel().0));
                    }
                    
                } else if error_lower.contains("permission denied") || 
                          error_lower.contains("access denied") ||
                          error_lower.contains("tcc") ||
                          error_lower.contains("declined") {
                    error!("🚫 Permission denied for audio device {}. Please check permissions.", device_name_clone);
                    
                    // For permission issues, try to continue but log the error
                    warn!("Continuing with reduced functionality due to permission issues");
                    
                } else if error_lower.contains("timeout") || 
                          error_lower.contains("timed out") ||
                          error_lower.contains("connection lost") {
                    warn!("⏰ Audio stream timeout for device {}, attempting recovery...", device_name_clone);
                    
                    // For timeout issues, mark as disconnected for reconnection attempt
                    is_disconnected_clone.store(true, Ordering::Relaxed);
                    
                } else {
                    error!("⚠️ Audio stream error on device {}: {}", device_name_clone, error_msg);
                    
                    // For other errors, check if they're recoverable
                    if error_lower.contains("buffer") || 
                       error_lower.contains("overflow") ||
                       error_lower.contains("underflow") {
                        warn!("🔄 Buffer-related error, attempting to continue...");
                        // These are usually recoverable, continue operation
                    } else {
                        // For unknown errors, mark as disconnected for potential reconnection
                        warn!("🔄 Unknown error type, marking device as disconnected for recovery");
                        is_disconnected_clone.store(true, Ordering::Relaxed);
                    }
                }
            };

            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => {
                    let managed_channel_f32 = managed_channel.clone();
                    match cpal_audio_device.build_input_stream(
                        &config.into(),
                        move |data: &[f32], _: &_| {
                            log::debug!("Audio callback triggered (F32)");
                            if let Some(arc) = is_running_weak_for_data.upgrade() {
                                if !arc.load(Ordering::Relaxed) {
                                    log::debug!("Audio callback: is_running is false, returning early (F32)");
                                    return;
                                }
                            } else {
                                log::debug!("Audio callback: is_running Arc was dropped, returning early (F32)");
                                return;
                            }
                            let mono = audio_to_mono(data, channels);
                            debug!("Received audio chunk: {} samples", mono.len());
                            
                            // Send directly to broadcast channel (sync operation)
                            if let Err(e) = broadcast_sender_clone.send(mono) {
                                warn!("Failed to send audio data: {}", e);
                            }
                        },
                        error_callback.clone(),
                        None,
                    ) {
                        Ok(stream) => stream,
                        Err(e) => {
                            error!("Failed to build input stream: {}", e);
                            return;
                        }
                    }
                }
                cpal::SampleFormat::I16 => {
                    let managed_channel_i16 = managed_channel.clone();
                    match cpal_audio_device.build_input_stream(
                        &config.into(),
                        move |data: &[i16], _: &_| {
                            log::debug!("Audio callback triggered (I16)");
                            if let Some(arc) = is_running_weak_for_data.upgrade() {
                                if !arc.load(Ordering::Relaxed) {
                                    log::debug!("Audio callback: is_running is false, returning early (I16)");
                                    return;
                                }
                            } else {
                                log::debug!("Audio callback: is_running Arc was dropped, returning early (I16)");
                                return;
                            }
                            let mono = audio_to_mono(bytemuck::cast_slice(data), channels);
                            debug!("Received audio chunk: {} samples", mono.len());
                            
                            // Send directly to broadcast channel (sync operation)
                            if let Err(e) = broadcast_sender_clone.send(mono) {
                                warn!("Failed to send audio data: {}", e);
                            }
                        },
                        error_callback.clone(),
                        None,
                    ) {
                        Ok(stream) => stream,
                        Err(e) => {
                            error!("Failed to build input stream: {}", e);
                            return;
                        }
                    }
                }
                cpal::SampleFormat::I32 => {
                    let managed_channel_i32 = managed_channel.clone();
                    match cpal_audio_device.build_input_stream(
                        &config.into(),
                        move |data: &[i32], _: &_| {
                            log::debug!("Audio callback triggered (I32)");
                            if let Some(arc) = is_running_weak_for_data.upgrade() {
                                if !arc.load(Ordering::Relaxed) {
                                    log::debug!("Audio callback: is_running is false, returning early (I32)");
                                    return;
                                }
                            } else {
                                log::debug!("Audio callback: is_running Arc was dropped, returning early (I32)");
                                return;
                            }
                            let mono = audio_to_mono(bytemuck::cast_slice(data), channels);
                            debug!("Received audio chunk: {} samples", mono.len());
                            
                            // Send directly to broadcast channel (sync operation)
                            if let Err(e) = broadcast_sender_clone.send(mono) {
                                warn!("Failed to send audio data: {}", e);
                            }
                        },
                        error_callback.clone(),
                        None,
                    ) {
                        Ok(stream) => stream,
                        Err(e) => {
                            error!("Failed to build input stream: {}", e);
                            return;
                        }
                    }
                }
                cpal::SampleFormat::I8 => {
                    let managed_channel_i8 = managed_channel.clone();
                    match cpal_audio_device.build_input_stream(
                        &config.into(),
                        move |data: &[i8], _: &_| {
                            log::debug!("Audio callback triggered (I8)");
                            if let Some(arc) = is_running_weak_for_data.upgrade() {
                                if !arc.load(Ordering::Relaxed) {
                                    log::debug!("Audio callback: is_running is false, returning early (I8)");
                                    return;
                                }
                            } else {
                                log::debug!("Audio callback: is_running Arc was dropped, returning early (I8)");
                                return;
                            }
                            let mono = audio_to_mono(bytemuck::cast_slice(data), channels);
                            debug!("Received audio chunk: {} samples", mono.len());
                            
                            // Send directly to broadcast channel (sync operation)
                            if let Err(e) = broadcast_sender_clone.send(mono) {
                                warn!("Failed to send audio data: {}", e);
                            }
                        },
                        error_callback.clone(),
                        None,
                    ) {
                        Ok(stream) => stream,
                        Err(e) => {
                            error!("Failed to build input stream: {}", e);
                            return;
                        }
                    }
                }
                _ => {
                    error!("unsupported sample format: {}", config.sample_format());
                    return;
                }
            };

            if let Err(e) = stream.play() {
                error!("failed to play stream for {}: {}", device.to_string(), e);
                let err_str = e.to_string().to_lowercase();
                if err_str.contains("permission") {
                    error!("Permission error detected. Please check microphone permissions");

                } else if err_str.contains("busy") {
                    error!("Device is busy. Another application might be using it");
                }
                return;
            }
            info!("Audio stream started successfully for device: {}", device_name);
            match stream_control_rx.recv() {
                Ok(StreamControl::Stop(response)) => {
                    info!("stopping audio stream...");
                    // First stop the stream
                    if let Err(e) = stream.pause() {
                        error!("failed to pause stream: {}", e);
                    }
                    // Close the stream to release OS resources
                    drop(stream);
                    // Signal completion
                    response.send(()).ok();
                    info!("audio stream stopped and cleaned up");
                }
                Ok(StreamControl::Recover(response)) => {
                    info!("🔄 Recovery signal received, attempting to restart audio stream...");
                    
                    // Pause current stream
                    if let Err(e) = stream.pause() {
                        warn!("failed to pause stream during recovery: {}", e);
                    }
                    
                    // Try to restart the stream
                    match stream.play() {
                        Ok(_) => {
                            info!("✅ Audio stream recovered successfully");
                            response.send(()).ok();
                        }
                        Err(e) => {
                            error!("❌ Failed to recover audio stream: {}", e);
                            // If recovery fails, fall back to stop
                            drop(stream);
                            response.send(()).ok();
                        }
                    }
                }
                Err(e) => {
                    warn!("Stream control channel error: {}", e);
                    return;
                }
            }
        }))));

        Ok(AudioStream {
            device,
            device_config: config,
            managed_channel,
            broadcast_sender,
            stream_control: stream_control_tx,
            stream_thread: Some(stream_thread),
            is_disconnected,
        })
    }

    pub async fn subscribe(&self) -> Result<broadcast::Receiver<Vec<f32>>> {
        Ok(self.broadcast_sender.subscribe())
    }

    pub async fn stop(&self) -> Result<()> {
        // Mark as disconnected first
        self.is_disconnected.store(true, Ordering::Release);
        
        // Close managed channel first
        if let Err(e) = self.managed_channel.close().await {
            warn!("Failed to close managed channel: {}", e);
        }
        
        // Send stop signal and wait for confirmation
        let (tx, _rx) = oneshot::channel();
        self.stream_control.send(StreamControl::Stop(tx))?;

        // Wait for thread to finish
        if let Some(thread_arc) = &self.stream_thread {
            let thread_arc = thread_arc.clone();
            let thread_handle = tokio::task::spawn_blocking(move || {
                let mut thread_guard = thread_arc.blocking_lock();
                if let Some(join_handle) = thread_guard.take() {
                    join_handle
                        .join()
                        .map_err(|_| anyhow!("failed to join stream thread"))
                } else {
                    Ok(())
                }
            });

            thread_handle.await??;
        }

        Ok(())
    }

    /// Attempt to recover the audio stream after an error
    pub async fn attempt_recovery(&self) -> Result<bool> {
        info!("🔄 Attempting to recover audio stream for device: {}", self.device.name);
        
        // Use managed channel's built-in recovery system
        match self.managed_channel.initiate_recovery().await {
            Ok(_) => {
                info!("✅ Managed channel recovery initiated successfully");
                // Reset disconnected flag
                self.is_disconnected.store(false, Ordering::Release);
                Ok(true)
            }
            Err(e) => {
                warn!("Managed channel recovery failed: {}", e);
                Ok(false)
            }
        }
    }
    
    /// Get channel health status
    pub async fn channel_health(&self) -> super::channel::ChannelHealthMetrics {
        self.managed_channel.health_metrics().await
    }
    
    /// Check if channel is healthy
    pub async fn is_channel_healthy(&self) -> bool {
        self.managed_channel.is_healthy().await
    }
}

#[cfg(target_os = "windows")]
fn get_windows_device(audio_device: &AudioDevice) -> Result<(cpal::Device, cpal::SupportedStreamConfig)> {
    let wasapi_host = cpal::host_from_id(cpal::HostId::Wasapi)
        .map_err(|e| anyhow!("Failed to create WASAPI host: {}", e))?;

    // Extract the base device name without the (input) or (output) suffix
    let base_name = if audio_device.name.ends_with(" (input)") {
        audio_device.name.trim_end_matches(" (input)")
    } else if audio_device.name.ends_with(" (output)") {
        audio_device.name.trim_end_matches(" (output)")
    } else {
        &audio_device.name
    };
    
    info!("Looking for Windows device with base name: {}", base_name);

    match audio_device.device_type {
        DeviceType::Input => {
            for device in wasapi_host.input_devices()? {
                if let Ok(name) = device.name() {
                    info!("Checking input device: {}", name);
                    // Check if the device name contains our base name
                    if name == base_name || name.contains(base_name) {
                        info!("Found matching input device: {}", name);
                        
                        // Try to get default input config with better error logging
                        match device.default_input_config() {
                            Ok(default_config) => {
                                info!("Using default input config: {:?}", default_config);
                                return Ok((device, default_config));
                            },
                            Err(e) => {
                                warn!("Failed to get default input config: {}. Trying supported configs...", e);
                                
                                // Try to find a supported configuration
                                if let Ok(supported_configs) = device.supported_input_configs() {
                                    let mut configs: Vec<_> = supported_configs.collect();
                                    if configs.is_empty() {
                                        warn!("No supported input configurations found for device: {}", name);
                                    } else {
                                        info!("Found {} supported input configurations", configs.len());
                                        
                                        // First try to find F32 format with 2 channels (stereo)
                                        for config in &configs {
                                            if config.sample_format() == cpal::SampleFormat::F32 && config.channels() == 2 {
                                                let config = config.with_max_sample_rate();
                                                info!("Using stereo F32 input config: {:?}", config);
                                                return Ok((device, config));
                                            }
                                        }
                                        
                                        // Then try any F32 format
                                        for config in &configs {
                                            if config.sample_format() == cpal::SampleFormat::F32 {
                                                let config = config.with_max_sample_rate();
                                                info!("Using F32 input config: {:?}", config);
                                                return Ok((device, config));
                                            }
                                        }
                                        
                                        // Finally, use the first available config
                                        let config = configs[0].with_max_sample_rate();
                                        info!("Using fallback input config: {:?}", config);
                                        return Ok((device, config));
                                    }
                                } else {
                                    warn!("Could not enumerate supported configurations for device: {}", name);
                                }
                                
                                return Err(anyhow!("No compatible input configuration found for device: {}", name));
                            }
                        }
                    }
                }
            }
            
            // If we didn't find a matching device, try the default input device as fallback
            info!("No matching input device found, trying default input device");
            if let Some(default_device) = wasapi_host.default_input_device() {
                if let Ok(name) = default_device.name() {
                    info!("Using default input device: {}", name);
                    if let Ok(config) = default_device.default_input_config() {
                        return Ok((default_device, config));
                    } else if let Ok(supported_configs) = default_device.supported_input_configs() {
                        if let Some(config) = supported_configs.into_iter().next() {
                            return Ok((default_device, config.with_max_sample_rate()));
                        }
                    }
                }
            }
        }
        DeviceType::Output => {
            for device in wasapi_host.output_devices()? {
                if let Ok(name) = device.name() {
                    info!("Checking output device: {}", name);
                    // Check if the device name contains our base name
                    if name == base_name || name.contains(base_name) {
                        info!("Found matching output device: {}", name);
                        
                        // For output devices, we want to use them in loopback mode
                        if let Ok(supported_configs) = device.supported_output_configs() {
                            let mut configs: Vec<_> = supported_configs.collect();
                            if configs.is_empty() {
                                warn!("No supported output configurations found for device: {}", name);
                            } else {
                                info!("Found {} supported output configurations", configs.len());
                                
                                // Try to find a config that supports f32 format with 2 channels (stereo)
                                for config in &configs {
                                    if config.sample_format() == cpal::SampleFormat::F32 && config.channels() == 2 {
                                        let config = config.with_max_sample_rate();
                                        info!("Using stereo F32 output config: {:?}", config);
                                        return Ok((device, config));
                                    }
                                }
                                
                                // Then try any F32 format
                                for config in &configs {
                                    if config.sample_format() == cpal::SampleFormat::F32 {
                                        let config = config.with_max_sample_rate();
                                        info!("Using F32 output config: {:?}", config);
                                        return Ok((device, config));
                                    }
                                }
                                
                                // Finally, use the first available config
                                let config = configs[0].with_max_sample_rate();
                                info!("Using fallback output config: {:?}", config);
                                return Ok((device, config));
                            }
                        } else {
                            warn!("Could not enumerate supported configurations for device: {}", name);
                        }
                        
                        // If we couldn't get supported configs, try default
                        if let Ok(default_config) = device.default_output_config() {
                            info!("Using default output config: {:?}", default_config);
                            return Ok((device, default_config));
                        }
                    }
                }
            }
            
            // If we didn't find a matching device, try the default output device as fallback
            info!("No matching output device found, trying default output device");
            if let Some(default_device) = wasapi_host.default_output_device() {
                if let Ok(name) = default_device.name() {
                    info!("Using default output device: {}", name);
                    if let Ok(config) = default_device.default_output_config() {
                        return Ok((default_device, config));
                    } else if let Ok(supported_configs) = default_device.supported_output_configs() {
                        if let Some(config) = supported_configs.into_iter().next() {
                            return Ok((default_device, config.with_max_sample_rate()));
                        }
                    }
                }
            }
        }
    }

    Err(anyhow!("Device not found or no compatible configuration available: {}", audio_device.name))
}

pub async fn get_device_and_config(
    audio_device: &AudioDevice,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig)> {
    #[cfg(target_os = "windows")]
    {
        return get_windows_device(audio_device);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let host = cpal::default_host();
        
        match audio_device.device_type {
            DeviceType::Input => {
                for device in host.input_devices()? {
                    if let Ok(name) = device.name() {
                        if name == audio_device.name {
                            let default_config = device
                                .default_input_config()
                                .map_err(|e| anyhow!("Failed to get default input config: {}", e))?;
                            return Ok((device, default_config));
                        }
                    }
                }
            }
            DeviceType::Output => {
                #[cfg(target_os = "macos")]
                {
                    if let Ok(host) = cpal::host_from_id(cpal::HostId::ScreenCaptureKit) {
                        for device in host.input_devices()? {
                            if let Ok(name) = device.name() {
                                if name == audio_device.name {
                                    let default_config = device
                                        .default_input_config()
                                        .map_err(|e| anyhow!("Failed to get default input config: {}", e))?;
                                    return Ok((device, default_config));
                                }
                            }
                        }
                    }
                }

                #[cfg(target_os = "linux")]
                {
                    // For Linux, we use PulseAudio monitor sources for system audio
                    if let Ok(pulse_host) = cpal::host_from_id(cpal::HostId::Pulse) {
                        for device in pulse_host.input_devices()? {
                            if let Ok(name) = device.name() {
                                if name == audio_device.name {
                                    let default_config = device
                                        .default_input_config()
                                        .map_err(|e| anyhow!("Failed to get default input config: {}", e))?;
                                    return Ok((device, default_config));
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Err(anyhow!("Device not found: {}", audio_device.name))
    }
}
