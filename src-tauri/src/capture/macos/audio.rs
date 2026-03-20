//! macOS audio device enumeration.
//!
//! This module provides audio device enumeration using Core Audio APIs.
//!
//! ## Architecture
//!
//! Audio enumeration uses Core Audio's AudioHardware APIs to list all audio
//! devices on the system, filtering by input (microphones) and output (speakers)
//! capabilities.

use crate::capture::error::EnumerationError;
use crate::capture::{AudioSource, AudioSourceType};

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use coreaudio_sys::{
    kAudioDevicePropertyDeviceNameCFString, kAudioDevicePropertyScopeInput,
    kAudioDevicePropertyScopeOutput, kAudioDevicePropertyStreams, kAudioHardwarePropertyDevices,
    kAudioObjectPropertyElementMain, kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject,
    AudioDeviceID, AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize,
    AudioObjectPropertyAddress, OSStatus,
};
use std::ffi::c_void;

/// Check if running on macOS 13 (Ventura) or later.
///
/// ScreenCaptureKit audio capture requires macOS 13+.
fn is_macos_13_or_later() -> bool {
    // Use sysctl to get macOS version
    use std::process::Command;

    let output = Command::new("sw_vers").arg("-productVersion").output().ok();

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

    tracing::debug!(
        "[Audio] Enumerated {} audio sources on macOS",
        sources.len()
    );
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
        AudioObjectGetPropertyDataSize(
            device_id,
            &property_address,
            0,
            std::ptr::null(),
            &mut data_size,
        )
    };

    status == 0 && data_size > 0
}

/// Initialize the audio backend.
///
/// Performs one-time setup and logs available audio devices.
#[allow(dead_code)]
pub fn init_audio_backend() -> Result<(), String> {
    tracing::debug!("[Audio] Initializing macOS audio backend");

    // Check macOS version for audio capture support
    if is_macos_13_or_later() {
        tracing::debug!("[Audio] macOS 13+ detected - system audio capture available");
    } else {
        tracing::debug!("[Audio] macOS version < 13 - system audio capture not available");
    }

    // Enumerate devices for logging
    match list_audio_sources() {
        Ok(sources) => {
            let output_count = sources
                .iter()
                .filter(|s| s.source_type == AudioSourceType::Output)
                .count();
            let input_count = sources
                .iter()
                .filter(|s| s.source_type == AudioSourceType::Input)
                .count();
            tracing::debug!(
                "[Audio] Found {} audio devices ({} outputs, {} inputs)",
                sources.len(),
                output_count,
                input_count
            );
        }
        Err(e) => {
            tracing::debug!("[Audio] Failed to enumerate audio devices: {:?}", e);
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
        tracing::debug!("System audio available: {}", result);
    }
}
