#[cfg(target_os = "macos")]
use std::sync::Arc;
use anyhow::{anyhow, Result};
use tokio::sync::broadcast;

#[cfg(target_os = "macos")]
use cidre::{core_audio as ca, cf, cat, av, os, arc, ns};

use super::{AudioDevice, DeviceType};

#[cfg(target_os = "macos")]
pub struct CoreAudioSystemTap {
    tap: ca::TapGuard,
    agg_desc: arc::Retained<cf::DictionaryOf<cf::String, cf::Type>>,
    device_name: String,
}

#[cfg(target_os = "macos")]
pub struct CoreAudioSystemStream {
    transmitter: Arc<broadcast::Sender<Vec<f32>>>,
    _device: ca::hardware::StartedDevice<ca::AggregateDevice>,
    _ctx: Box<StreamCtx>,
    _tap: ca::TapGuard,
}

#[cfg(target_os = "macos")]
struct StreamCtx {
    format: arc::R<av::AudioFormat>,
    tx: broadcast::Sender<Vec<f32>>,
    buffer: Vec<f32>,
}

#[cfg(target_os = "macos")]
impl CoreAudioSystemTap {
    pub fn new() -> Result<Self> {
        log::info!("Creating CoreAudio Process Tap for system audio");
        
        // Get the default output device (what's currently playing audio)
        let output_device = ca::System::default_output_device()
            .map_err(|e| anyhow!("Failed to get default output device: {}", e))?;
        let output_uid = output_device.uid()
            .map_err(|e| anyhow!("Failed to get output device UID: {}", e))?;
        
        let device_name = output_device.name()
            .unwrap_or("Unknown Speaker".into())
            .to_string();
        
        log::info!("System audio device: {} (using CoreAudio Process Tap)", device_name);
        log::info!("Nominal sample rate: {:?}", output_device.nominal_sample_rate());
        
        // Create a subprocess dictionary for the output device
        let sub_device = cf::DictionaryOf::with_keys_values(
            &[ca::sub_device_keys::uid()],
            &[output_uid.as_type_ref()],
        );
        
        // Create a global process tap (captures all system audio)
        let tap_desc = ca::TapDesc::with_mono_global_tap_excluding_processes(&ns::Array::new());
        let tap = tap_desc.create_process_tap()
            .map_err(|e| anyhow!("Failed to create process tap: {}", e))?;
        
        // Create a subprocess dictionary for the tap
        let sub_tap = cf::DictionaryOf::with_keys_values(
            &[ca::sub_device_keys::uid()],
            &[tap.uid().unwrap().as_type_ref()],
        );
        
        // Create an aggregate device that combines the output device and the tap
        let agg_desc = cf::DictionaryOf::with_keys_values(
            &[
                ca::aggregate_device_keys::is_private(),
                ca::aggregate_device_keys::is_stacked(),
                ca::aggregate_device_keys::tap_auto_start(),
                ca::aggregate_device_keys::name(),
                ca::aggregate_device_keys::main_sub_device(),
                ca::aggregate_device_keys::uid(),
                ca::aggregate_device_keys::sub_device_list(),
                ca::aggregate_device_keys::tap_list(),
            ],
            &[
                cf::Boolean::value_true().as_type_ref(),
                cf::Boolean::value_false(),
                cf::Boolean::value_true(),
                cf::str!(c"Meetily-System-Audio-Tap"),
                &output_uid,
                &cf::Uuid::new().to_cf_string(),
                &cf::ArrayOf::from_slice(&[sub_device.as_ref()]),
                &cf::ArrayOf::from_slice(&[sub_tap.as_ref()]),
            ],
        );
        
        log::info!("CoreAudio Process Tap created successfully");
        
        Ok(Self {
            tap,
            agg_desc,
            device_name,
        })
    }
    
    pub fn create_stream(self) -> Result<CoreAudioSystemStream> {
        log::info!("Starting CoreAudio system audio stream");
        
        // Get audio format from the tap
        let asbd = self.tap.asbd()
            .map_err(|e| anyhow!("Failed to get audio format from tap: {}", e))?;
        let format = av::AudioFormat::with_asbd(&asbd)
            .ok_or_else(|| anyhow!("Failed to create audio format"))?;
        
        log::info!("System audio format: sample_rate={}, channels={}", 
                  asbd.sample_rate, asbd.channels_per_frame);
        
        // Create broadcast channel for audio data
        let (tx, _) = broadcast::channel::<Vec<f32>>(1000);
        let tx_clone = tx.clone();
        
        // Create context for the audio callback
        let mut ctx = Box::new(StreamCtx {
            format,
            tx,
            buffer: Vec::with_capacity(8192),
        });
        
        // Create and start the aggregate device
        let agg_device = ca::AggregateDevice::with_desc(&self.agg_desc)
            .map_err(|e| anyhow!("Failed to create aggregate device: {}", e))?;
        
        // Create IO proc for handling audio data
        extern "C" fn audio_proc(
            _device: ca::Device,
            _now: &cat::AudioTimeStamp,
            _input_data: &cat::AudioBufList<1>,
            _input_time: &cat::AudioTimeStamp,
            output_data: &mut cat::AudioBufList<1>,
            _output_time: &cat::AudioTimeStamp,
            ctx: Option<&mut StreamCtx>,
        ) -> os::Status {
            let ctx = match ctx {
                Some(ctx) => ctx,
                None => return os::Status::NO_ERR,
            };
            
            // Ensure we're working with F32 PCM format
            if ctx.format.common_format() != av::audio::CommonFormat::PcmF32 {
                log::warn!("Unexpected audio format in CoreAudio callback");
                return os::Status::NO_ERR;
            }
            
            // Create audio buffer view for OUTPUT data (system audio being played)
            if let Some(view) = av::AudioPcmBuf::with_buf_list_no_copy(&ctx.format, output_data, None) {
                if let Some(data) = view.data_f32_at(0) {
                    // Convert to Vec<f32> and send through broadcast channel
                    let audio_chunk = data.to_vec();
                    
                    // Only send if we have actual audio data (not silence)
                    let max_amplitude = audio_chunk.iter().fold(0.0f32, |acc, &x| acc.max(x.abs()));
                    let chunk_len = audio_chunk.len();
                    
                    if max_amplitude > 0.0001 { // Threshold to avoid sending pure silence
                        if let Err(_) = ctx.tx.send(audio_chunk) {
                            log::debug!("No receivers for system audio data");
                        } else {
                            log::debug!("🎵 Sent system audio chunk: {} samples, max: {:.6}", chunk_len, max_amplitude);
                        }
                    } else {
                        log::debug!("🔇 System audio too quiet (max: {:.6}), skipping", max_amplitude);
                    }
                } else {
                    log::debug!("No F32 data in CoreAudio output buffer");
                }
            } else {
                log::debug!("Failed to create audio buffer view for output data");
            }
            
            os::Status::NO_ERR
        }
        
        // Register the IO proc with the aggregate device
        let proc_id = agg_device.create_io_proc_id(audio_proc, Some(&mut *ctx))
            .map_err(|e| anyhow!("Failed to create IO proc: {}", e))?;
        
        log::info!("✅ IO proc registered with ID: {:?}", proc_id);
        
        // Start the device
        let started_device = ca::device_start(agg_device, Some(proc_id))
            .map_err(|e| anyhow!("Failed to start aggregate device: {}", e))?;
        
        log::info!("✅ CoreAudio system audio stream started successfully");
        
        Ok(CoreAudioSystemStream {
            transmitter: Arc::new(tx_clone),
            _device: started_device,
            _ctx: ctx,
            _tap: self.tap,
        })
    }
    
    pub fn device_name(&self) -> &str {
        &self.device_name
    }
}

#[cfg(target_os = "macos")]
impl CoreAudioSystemStream {
    pub async fn subscribe(&self) -> broadcast::Receiver<Vec<f32>> {
        self.transmitter.subscribe()
    }
    
    pub async fn stop(&self) -> Result<()> {
        log::info!("Stopping CoreAudio system audio stream");
        // The device will be automatically stopped when dropped
        Ok(())
    }
}

// Fallback implementation for non-macOS platforms
#[cfg(not(target_os = "macos"))]
pub struct CoreAudioSystemTap;

#[cfg(not(target_os = "macos"))]
pub struct CoreAudioSystemStream;

#[cfg(not(target_os = "macos"))]
impl CoreAudioSystemTap {
    pub fn new() -> Result<Self> {
        Err(anyhow!("CoreAudio Process Tap is only available on macOS"))
    }
    
    pub fn create_stream(self) -> Result<CoreAudioSystemStream> {
        Err(anyhow!("CoreAudio Process Tap is only available on macOS"))
    }
    
    pub fn device_name(&self) -> &str {
        "Not Available"
    }
}

#[cfg(not(target_os = "macos"))]
impl CoreAudioSystemStream {
    pub async fn subscribe(&self) -> broadcast::Receiver<Vec<f32>> {
        let (_, rx) = broadcast::channel(1);
        rx
    }
    
    pub async fn stop(&self) -> Result<()> {
        Ok(())
    }
}

/// Create a system audio device using CoreAudio Process Tap
pub fn create_coreaudio_system_device() -> Result<AudioDevice> {
    #[cfg(target_os = "macos")]
    {
        let tap = CoreAudioSystemTap::new()?;
        let device_name = format!("{} (CoreAudio Tap)", tap.device_name());
        Ok(AudioDevice::new(device_name, DeviceType::Output))
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        Err(anyhow!("CoreAudio Process Tap is only supported on macOS"))
    }
}

/// Create a CoreAudio system audio stream
pub fn create_coreaudio_system_stream() -> Result<(CoreAudioSystemTap, AudioDevice)> {
    let tap = CoreAudioSystemTap::new()?;
    let device_name = format!("{} (CoreAudio Tap)", tap.device_name());
    let device = AudioDevice::new(device_name, DeviceType::Output);
    Ok((tap, device))
}