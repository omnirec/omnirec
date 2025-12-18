//! macOS audio device enumeration and system audio capture.
//!
//! This module provides:
//! - Audio device enumeration using Core Audio APIs
//! - System audio capture using ScreenCaptureKit (macOS 13+)
//!
//! ## Architecture
//!
//! Audio enumeration uses Core Audio's AudioHardware APIs to list all audio
//! devices on the system, filtering by input (microphones) and output (speakers)
//! capabilities.
//!
//! System audio capture uses ScreenCaptureKit's audio capture feature, which
//! captures all system audio globally (not per-device). This requires macOS 13+.
//!
//! ## Limitations
//!
//! - System audio capture is only available on macOS 13+
//! - ScreenCaptureKit captures all system audio, not per-device
//! - Microphone capture and dual audio mixing are deferred to future changes

use crate::capture::error::{CaptureError, EnumerationError};
use crate::capture::types::{AudioReceiver, AudioSample, AudioSource, AudioSourceType, StopHandle};

use coreaudio_sys::{
    kAudioDevicePropertyDeviceNameCFString, kAudioDevicePropertyScopeInput,
    kAudioDevicePropertyScopeOutput, kAudioDevicePropertyStreams, kAudioHardwarePropertyDevices,
    kAudioObjectPropertyElementMain, kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject,
    AudioDeviceID, AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize,
    AudioObjectPropertyAddress, OSStatus,
};
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use screencapturekit_sys::{
    cm_sample_buffer_ref::CMSampleBufferRef,
    content_filter::{UnsafeContentFilter, UnsafeInitParams},
    os_types::{base::BOOL, rc::Id},
    shareable_content::UnsafeSCShareableContent,
    stream::UnsafeSCStream,
    stream_configuration::UnsafeStreamConfiguration,
    stream_error_handler::UnsafeSCStreamError,
    stream_output_handler::UnsafeSCStreamOutput,
};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Target sample rate for audio output (matches encoder expectations).
const TARGET_SAMPLE_RATE: u32 = 48000;

/// Target channel count for audio output.
const TARGET_CHANNELS: u32 = 2;

/// Simple linear resampler for converting audio sample rates.
/// Uses linear interpolation for simplicity - sufficient for voice/system audio.
fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32, channels: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let channels = channels as usize;
    let input_frames = samples.len() / channels;
    let output_frames = ((input_frames as f64) / ratio).ceil() as usize;
    
    let mut output = Vec::with_capacity(output_frames * channels);
    
    for out_frame in 0..output_frames {
        let in_pos = out_frame as f64 * ratio;
        let in_frame = in_pos.floor() as usize;
        let frac = (in_pos - in_frame as f64) as f32;
        
        for ch in 0..channels {
            let idx0 = in_frame * channels + ch;
            let idx1 = ((in_frame + 1).min(input_frames - 1)) * channels + ch;
            
            if idx0 < samples.len() && idx1 < samples.len() {
                // Linear interpolation between adjacent samples
                let s0 = samples[idx0];
                let s1 = samples[idx1];
                output.push(s0 + frac * (s1 - s0));
            } else if idx0 < samples.len() {
                output.push(samples[idx0]);
            }
        }
    }
    
    output
}

/// Check if running on macOS 13 (Ventura) or later.
///
/// ScreenCaptureKit audio capture requires macOS 13+.
fn is_macos_13_or_later() -> bool {
    // Use sysctl to get macOS version
    use std::process::Command;
    
    let output = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok();
    
    if let Some(output) = output {
        if let Ok(version_str) = String::from_utf8(output.stdout) {
            let version_str = version_str.trim();
            // Parse version like "13.0" or "14.2.1"
            let parts: Vec<&str> = version_str.split('.').collect();
            if let Some(major_str) = parts.first() {
                if let Ok(major) = major_str.parse::<u32>() {
                    return major >= 13;
                }
            }
        }
    }
    
    // Default to false if we can't determine version
    false
}

// =============================================================================
// Audio Device Enumeration
// =============================================================================

/// List all available audio sources using Core Audio.
///
/// Returns both output devices (for system audio identification) and input
/// devices (microphones). Note that on macOS, ScreenCaptureKit captures all
/// system audio globally, so the output device list is informational only.
pub fn list_audio_sources() -> Result<Vec<AudioSource>, EnumerationError> {
    let mut sources = Vec::new();

    // Get all audio device IDs
    let device_ids = get_audio_device_ids()?;

    for device_id in device_ids {
        // Get device name
        let name = match get_device_name(device_id) {
            Ok(n) => n,
            Err(_) => continue, // Skip devices without names
        };

        // Check if device has output streams (speakers, headphones)
        if has_output_streams(device_id) {
            sources.push(AudioSource {
                id: device_id.to_string(),
                name: format!("{} (Monitor)", name),
                source_type: AudioSourceType::Output,
            });
        }

        // Check if device has input streams (microphones)
        if has_input_streams(device_id) {
            sources.push(AudioSource {
                id: device_id.to_string(),
                name: name.clone(),
                source_type: AudioSourceType::Input,
            });
        }
    }

    eprintln!("[Audio] Enumerated {} audio sources on macOS", sources.len());
    Ok(sources)
}

/// Get all audio device IDs from Core Audio.
fn get_audio_device_ids() -> Result<Vec<AudioDeviceID>, EnumerationError> {
    let property_address = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDevices,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };

    // Get the size of the property data
    let mut data_size: u32 = 0;
    let status: OSStatus = unsafe {
        AudioObjectGetPropertyDataSize(
            kAudioObjectSystemObject,
            &property_address,
            0,
            std::ptr::null(),
            &mut data_size,
        )
    };

    if status != 0 {
        return Err(EnumerationError::PlatformError(format!(
            "Failed to get audio device list size: OSStatus {}",
            status
        )));
    }

    if data_size == 0 {
        return Ok(Vec::new());
    }

    // Calculate number of devices
    let device_count = data_size as usize / std::mem::size_of::<AudioDeviceID>();
    let mut device_ids = vec![0 as AudioDeviceID; device_count];

    // Get the device IDs
    let status: OSStatus = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &property_address,
            0,
            std::ptr::null(),
            &mut data_size,
            device_ids.as_mut_ptr() as *mut c_void,
        )
    };

    if status != 0 {
        return Err(EnumerationError::PlatformError(format!(
            "Failed to get audio device IDs: OSStatus {}",
            status
        )));
    }

    Ok(device_ids)
}

/// Get the name of an audio device.
fn get_device_name(device_id: AudioDeviceID) -> Result<String, EnumerationError> {
    let property_address = AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyDeviceNameCFString,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };

    let mut name_ref: core_foundation::string::CFStringRef = std::ptr::null();
    let mut data_size: u32 = std::mem::size_of::<core_foundation::string::CFStringRef>() as u32;

    let status: OSStatus = unsafe {
        AudioObjectGetPropertyData(
            device_id,
            &property_address,
            0,
            std::ptr::null(),
            &mut data_size,
            &mut name_ref as *mut _ as *mut c_void,
        )
    };

    if status != 0 || name_ref.is_null() {
        return Err(EnumerationError::PlatformError(format!(
            "Failed to get device name: OSStatus {}",
            status
        )));
    }

    // Convert CFString to Rust String
    let cf_string: CFString = unsafe { CFString::wrap_under_get_rule(name_ref) };
    Ok(cf_string.to_string())
}

/// Check if a device has output streams (speakers, headphones).
fn has_output_streams(device_id: AudioDeviceID) -> bool {
    has_streams(device_id, kAudioDevicePropertyScopeOutput)
}

/// Check if a device has input streams (microphones).
fn has_input_streams(device_id: AudioDeviceID) -> bool {
    has_streams(device_id, kAudioDevicePropertyScopeInput)
}

/// Check if a device has streams in the specified scope.
fn has_streams(device_id: AudioDeviceID, scope: u32) -> bool {
    let property_address = AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyStreams,
        mScope: scope,
        mElement: kAudioObjectPropertyElementMain,
    };

    let mut data_size: u32 = 0;
    let status: OSStatus = unsafe {
        AudioObjectGetPropertyDataSize(device_id, &property_address, 0, std::ptr::null(), &mut data_size)
    };

    status == 0 && data_size > 0
}

// =============================================================================
// System Audio Capture
// =============================================================================

/// Error handler for SCStream.
struct AudioCaptureErrorHandler;

impl UnsafeSCStreamError for AudioCaptureErrorHandler {
    fn handle_error(&self) {
        eprintln!("[Audio] ScreenCaptureKit stream error");
    }
}

/// Audio output handler that converts CMSampleBuffer to AudioSample.
/// 
/// We use the unsafe UnsafeSCStreamOutput trait directly instead of the high-level
/// StreamOutput trait because the latter wraps sample buffers in CMSampleBuffer::new()
/// which calls get_frame_info() - this panics on audio samples since they don't have
/// frame attachments (that's video-only).
struct AudioOutputHandler {
    tx: mpsc::Sender<AudioSample>,
    stop_flag: Arc<AtomicBool>,
}

// Audio output type constant (matches SCStreamOutputType)
const SC_STREAM_OUTPUT_TYPE_AUDIO: u8 = 1;

impl UnsafeSCStreamOutput for AudioOutputHandler {
    fn did_output_sample_buffer(&self, sample: Id<CMSampleBufferRef>, of_type: u8) {
        // Only handle audio output (type 1)
        if of_type != SC_STREAM_OUTPUT_TYPE_AUDIO {
            return;
        }

        // Check if we should stop
        if self.stop_flag.load(Ordering::Relaxed) {
            return;
        }

        // First check if this sample buffer has valid audio format description
        // This is a safer way to check before calling get_av_audio_buffer_list()
        // which can panic on null pointers
        let format_desc = match sample.get_format_description() {
            Some(desc) => desc,
            None => {
                // No format description means no valid audio data
                return;
            }
        };

        // Check if this is actually an audio format
        let asbd = match format_desc.audio_format_description_get_stream_basic_description() {
            Some(desc) => desc,
            None => {
                // Not an audio format description
                return;
            }
        };

        let sample_rate = asbd.sample_rate as u32;
        if sample_rate == 0 {
            // Invalid sample rate
            return;
        }

        // Check format flags for non-interleaved audio
        // kAudioFormatFlagIsNonInterleaved = 32
        let is_non_interleaved = (asbd.format_flags & 32) != 0;
        let channel_count = asbd.channels_per_frame as usize;

        // Log first sample's format info for debugging
        static LOGGED_FORMAT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !LOGGED_FORMAT.swap(true, Ordering::Relaxed) {
            eprintln!("[Audio] Capture format: sample_rate={}, channels={}, format_id={}, format_flags={}, bytes_per_packet={}, frames_per_packet={}, bytes_per_frame={}, bits_per_channel={}, non_interleaved={}",
                asbd.sample_rate,
                asbd.channels_per_frame,
                asbd.format_id,
                asbd.format_flags,
                asbd.bytes_per_packet,
                asbd.frames_per_packet,
                asbd.bytes_per_frame,
                asbd.bits_per_channel,
                is_non_interleaved
            );
        }

        // Now try to get audio buffers - the format description check above
        // should ensure we have valid audio data
        let audio_buffers = sample.get_av_audio_buffer_list();

        if audio_buffers.is_empty() {
            return;
        }

        // Handle non-interleaved vs interleaved audio
        // Non-interleaved: each buffer contains one channel's samples
        // Interleaved: single buffer with samples alternating between channels
        let interleaved_samples: Vec<f32> = if is_non_interleaved && audio_buffers.len() >= 2 {
            // Non-interleaved stereo: interleave the two channel buffers
            let left_bytes = &audio_buffers[0].data;
            let right_bytes = &audio_buffers[1].data;
            
            let left_samples: &[f32] = unsafe {
                std::slice::from_raw_parts(
                    left_bytes.as_ptr() as *const f32,
                    left_bytes.len() / std::mem::size_of::<f32>()
                )
            };
            let right_samples: &[f32] = unsafe {
                std::slice::from_raw_parts(
                    right_bytes.as_ptr() as *const f32,
                    right_bytes.len() / std::mem::size_of::<f32>()
                )
            };
            
            // Interleave: L0, R0, L1, R1, L2, R2, ...
            let frame_count = left_samples.len().min(right_samples.len());
            let mut interleaved = Vec::with_capacity(frame_count * 2);
            for i in 0..frame_count {
                interleaved.push(left_samples[i]);
                interleaved.push(right_samples[i]);
            }
            interleaved
        } else if is_non_interleaved && audio_buffers.len() == 1 {
            // Non-interleaved mono: duplicate to stereo
            let mono_bytes = &audio_buffers[0].data;
            let mono_samples: &[f32] = unsafe {
                std::slice::from_raw_parts(
                    mono_bytes.as_ptr() as *const f32,
                    mono_bytes.len() / std::mem::size_of::<f32>()
                )
            };
            mono_samples.iter().flat_map(|&s| [s, s]).collect()
        } else {
            // Interleaved audio: collect all samples directly
            let mut all_samples: Vec<f32> = Vec::new();
            for buffer in &audio_buffers {
                let bytes = &buffer.data;
                let sample_count = bytes.len() / std::mem::size_of::<f32>();
                if sample_count > 0 {
                    let samples: &[f32] = unsafe {
                        std::slice::from_raw_parts(bytes.as_ptr() as *const f32, sample_count)
                    };
                    all_samples.extend_from_slice(samples);
                }
            }
            
            // Handle mono to stereo if needed
            if channel_count == 1 {
                all_samples.iter().flat_map(|&s| [s, s]).collect()
            } else {
                all_samples
            }
        };

        if interleaved_samples.is_empty() {
            return;
        }

        let converted_channels = 2u32; // Always output stereo

        // Resample to target rate if needed
        let final_samples = if sample_rate != TARGET_SAMPLE_RATE {
            resample_linear(&interleaved_samples, sample_rate, TARGET_SAMPLE_RATE, converted_channels)
        } else {
            interleaved_samples
        };

        // Create audio sample with TARGET rate (always 48kHz after resampling)
        let audio_sample = AudioSample {
            data: final_samples,
            sample_rate: TARGET_SAMPLE_RATE,
            channels: converted_channels,
        };

        // Send to channel (non-blocking)
        let _ = self.tx.try_send(audio_sample);
    }
}

/// Start system audio capture using ScreenCaptureKit.
///
/// Note: ScreenCaptureKit captures ALL system audio, not a specific device.
/// The source_id parameter is ignored on macOS.
///
/// Requires macOS 13+ and screen recording permission.
pub fn start_audio_capture(_source_id: &str) -> Result<(AudioReceiver, StopHandle), CaptureError> {
    // Check macOS version
    if !is_macos_13_or_later() {
        return Err(CaptureError::NotImplemented(
            "System audio capture requires macOS 13 or later".to_string(),
        ));
    }

    // Check screen recording permission
    if !super::MacOSBackend::has_screen_recording_permission() {
        // Try to trigger the permission prompt
        super::MacOSBackend::trigger_permission_prompt();
        
        return Err(CaptureError::PermissionDenied(
            "Screen recording permission required for audio capture. Please grant permission in System Settings > Privacy & Security > Screen Recording, then try again.".to_string(),
        ));
    }

    eprintln!("[Audio] Starting system audio capture on macOS");

    // Get shareable content using unsafe API
    let content = UnsafeSCShareableContent::get()
        .map_err(|e| CaptureError::AudioError(format!("Failed to get shareable content: {}", e)))?;

    // Get the first display for content filter (required even for audio-only capture)
    let display = content
        .displays()
        .into_iter()
        .next()
        .ok_or_else(|| CaptureError::AudioError("No display found".to_string()))?;

    let display_width = display.get_width();
    let display_height = display.get_height();

    // Create content filter for the display using unsafe API
    let filter = UnsafeContentFilter::init(UnsafeInitParams::Display(display));

    // Configure stream with audio enabled using unsafe API
    // We use minimal video settings since we only want audio
    let config = UnsafeStreamConfiguration {
        width: display_width.min(320), // Small size to minimize overhead
        height: display_height.min(240),
        captures_audio: BOOL::from(true),
        sample_rate: TARGET_SAMPLE_RATE,
        channel_count: TARGET_CHANNELS,
        excludes_current_process_audio: BOOL::from(true), // Don't capture our own app's audio
        shows_cursor: BOOL::from(false),
        ..Default::default()
    };

    // Create channel for audio samples
    let (tx, rx) = mpsc::channel(256);

    // Create stop flag
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();

    // Create stream using unsafe API directly
    // This avoids the high-level wrapper that causes panics on audio samples
    let stream = UnsafeSCStream::init(filter, config.into(), AudioCaptureErrorHandler);

    // Add audio output handler using unsafe API
    // Type 1 = Audio output
    let handler = AudioOutputHandler {
        tx,
        stop_flag: stop_flag.clone(),
    };
    stream.add_stream_output(handler, SC_STREAM_OUTPUT_TYPE_AUDIO);

    // Start capture
    stream
        .start_capture()
        .map_err(|e| CaptureError::AudioError(format!("Failed to start audio capture: {}", e)))?;

    eprintln!("[Audio] System audio capture started");

    // Keep stream alive in a thread
    std::thread::spawn(move || {
        while !stop_flag.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        eprintln!("[Audio] Stopping system audio capture");
        let _ = stream.stop_capture();
    });

    Ok((rx, stop_flag_clone))
}

/// Start system audio capture without requiring a source ID.
///
/// This is the preferred API for macOS since ScreenCaptureKit captures
/// all system audio globally.
pub fn start_system_audio_capture() -> Result<(AudioReceiver, StopHandle), CaptureError> {
    start_audio_capture("system")
}

/// Initialize the audio backend.
///
/// Performs one-time setup and logs available audio devices.
pub fn init_audio_backend() -> Result<(), String> {
    eprintln!("[Audio] Initializing macOS audio backend");

    // Check macOS version for audio capture support
    if is_macos_13_or_later() {
        eprintln!("[Audio] macOS 13+ detected - system audio capture available");
    } else {
        eprintln!("[Audio] macOS version < 13 - system audio capture not available");
    }

    // Enumerate devices for logging
    match list_audio_sources() {
        Ok(sources) => {
            let output_count = sources.iter().filter(|s| s.source_type == AudioSourceType::Output).count();
            let input_count = sources.iter().filter(|s| s.source_type == AudioSourceType::Input).count();
            eprintln!(
                "[Audio] Found {} audio devices ({} outputs, {} inputs)",
                sources.len(),
                output_count,
                input_count
            );
        }
        Err(e) => {
            eprintln!("[Audio] Failed to enumerate audio devices: {:?}", e);
        }
    }

    Ok(())
}

/// Check if system audio capture is available on this macOS version.
pub fn is_system_audio_available() -> bool {
    is_macos_13_or_later()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_source_type_serialization() {
        let input = AudioSourceType::Input;
        let output = AudioSourceType::Output;

        assert_eq!(serde_json::to_string(&input).unwrap(), "\"input\"");
        assert_eq!(serde_json::to_string(&output).unwrap(), "\"output\"");
    }

    #[test]
    fn test_list_audio_sources_returns_ok() {
        // Should return Ok even if no devices are found
        let result = list_audio_sources();
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_audio_backend_succeeds() {
        let result = init_audio_backend();
        assert!(result.is_ok());
    }

    #[test]
    fn test_macos_version_check() {
        // Just verify the function doesn't crash
        let _ = is_macos_13_or_later();
    }

    #[test]
    fn test_system_audio_available_check() {
        // Should return a boolean without crashing
        let result = is_system_audio_available();
        eprintln!("System audio available: {}", result);
    }
}
