//! Windows audio device enumeration using WASAPI.
//!
//! This module provides audio device enumeration via the Windows Audio Session API (WASAPI).
//! Audio capture is not yet implemented - only enumeration is supported.

use crate::capture::error::{CaptureError, EnumerationError};
use crate::capture::types::{AudioReceiver, AudioSource, AudioSourceType, StopHandle};

use windows::core::{PCWSTR, PROPVARIANT};
use windows::Win32::Media::Audio::{
    eCapture, eRender, IMMDevice, IMMDeviceCollection, IMMDeviceEnumerator, MMDeviceEnumerator,
    DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, PROPERTYKEY};

/// PKEY_Device_FriendlyName - the friendly name property key
/// {a45c254e-df1c-4efd-8020-67d146a850e0}, 14
const PKEY_DEVICE_FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY {
    fmtid: windows::core::GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
    pid: 14,
};

/// List all available audio sources using WASAPI.
///
/// Enumerates both output devices (for system audio loopback capture) and
/// input devices (microphones). Returns an empty list if enumeration fails.
pub fn list_audio_sources() -> Result<Vec<AudioSource>, EnumerationError> {
    // Initialize COM for this thread (MTA)
    // Safety: COM initialization is required for WASAPI calls
    let com_initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() };

    // Even if COM was already initialized, we should proceed
    // CoInitializeEx returns S_FALSE if already initialized, which is still success

    let result = enumerate_audio_devices();

    // Uninitialize COM if we initialized it
    if com_initialized {
        unsafe { CoUninitialize() };
    }

    result
}

/// Internal function to enumerate audio devices after COM is initialized.
fn enumerate_audio_devices() -> Result<Vec<AudioSource>, EnumerationError> {
    let mut sources = Vec::new();

    // Create the device enumerator
    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| {
            eprintln!("[Audio] Failed to create device enumerator: {:?}", e);
            EnumerationError::PlatformError(format!("Failed to create device enumerator: {}", e))
        })?
    };

    // Enumerate output devices (eRender) - these are used for system audio loopback
    if let Ok(output_devices) = enumerate_devices_by_flow(&enumerator, eRender) {
        for device in output_devices {
            sources.push(AudioSource {
                id: device.0,
                name: format!("{} (Monitor)", device.1),
                source_type: AudioSourceType::Output,
            });
        }
    }

    // Enumerate input devices (eCapture) - microphones
    if let Ok(input_devices) = enumerate_devices_by_flow(&enumerator, eCapture) {
        for device in input_devices {
            sources.push(AudioSource {
                id: device.0,
                name: device.1,
                source_type: AudioSourceType::Input,
            });
        }
    }

    eprintln!("[Audio] Enumerated {} audio devices", sources.len());
    Ok(sources)
}

/// Enumerate devices by data flow direction.
///
/// Returns a vector of (device_id, friendly_name) tuples.
fn enumerate_devices_by_flow(
    enumerator: &IMMDeviceEnumerator,
    data_flow: windows::Win32::Media::Audio::EDataFlow,
) -> Result<Vec<(String, String)>, EnumerationError> {
    let mut devices = Vec::new();

    // Get device collection for the specified flow direction
    let collection: IMMDeviceCollection = unsafe {
        enumerator
            .EnumAudioEndpoints(data_flow, DEVICE_STATE_ACTIVE)
            .map_err(|e| {
                EnumerationError::PlatformError(format!("Failed to enumerate endpoints: {}", e))
            })?
    };

    // Get device count
    let count = unsafe {
        collection.GetCount().map_err(|e| {
            EnumerationError::PlatformError(format!("Failed to get device count: {}", e))
        })?
    };

    // Iterate through devices
    for i in 0..count {
        if let Ok(device) = unsafe { collection.Item(i) } {
            if let Some((id, name)) = get_device_info(&device) {
                devices.push((id, name));
            }
        }
    }

    Ok(devices)
}

/// Extract device ID and friendly name from an IMMDevice.
fn get_device_info(device: &IMMDevice) -> Option<(String, String)> {
    // Get device ID
    let id = unsafe {
        let id_ptr = device.GetId().ok()?;
        let id_str = pcwstr_to_string(PCWSTR(id_ptr.0));
        // Free the allocated string
        windows::Win32::System::Com::CoTaskMemFree(Some(id_ptr.0 as *const _));
        id_str
    }?;

    // Get friendly name from device properties
    let name = get_device_friendly_name(device).unwrap_or_else(|| "Unknown Device".to_string());

    Some((id, name))
}

/// Get the friendly name of a device from its property store.
fn get_device_friendly_name(device: &IMMDevice) -> Option<String> {
    unsafe {
        // Open property store
        let store: IPropertyStore = device.OpenPropertyStore(STGM_READ).ok()?;

        // Get friendly name property
        let prop_value: PROPVARIANT = store.GetValue(&PKEY_DEVICE_FRIENDLY_NAME).ok()?;

        // Extract string from PROPVARIANT using the windows crate's built-in conversion
        propvariant_to_string(&prop_value)
    }
}

/// Convert a PROPVARIANT to a String if it contains a string value.
fn propvariant_to_string(pv: &PROPVARIANT) -> Option<String> {
    // The windows crate PROPVARIANT has a method to convert to string types
    // We can use the VT_LPWSTR type check and extract the wide string
    unsafe {
        // Try to get as a PWSTR (wide string)
        // The PROPVARIANT anonymous union contains the actual value
        // For VT_LPWSTR, it's stored in the pwszVal field

        // Use the windows crate's built-in display/debug to see if it's a string
        // or we can check the vt type directly
        let inner = &pv.as_raw().Anonymous.Anonymous;
        let vt = inner.vt;

        // VT_LPWSTR = 31
        if vt == 31 {
            let pwsz = inner.Anonymous.pwszVal;
            if !pwsz.is_null() {
                return pcwstr_to_string(PCWSTR(pwsz));
            }
        }
    }
    None
}

/// Convert a null-terminated wide string pointer to a Rust String.
fn pcwstr_to_string(pcwstr: PCWSTR) -> Option<String> {
    if pcwstr.is_null() {
        return None;
    }

    unsafe {
        // Find string length
        let mut len = 0;
        while *pcwstr.0.add(len) != 0 {
            len += 1;
        }

        if len == 0 {
            return Some(String::new());
        }

        // Convert to Rust string
        let slice = std::slice::from_raw_parts(pcwstr.0, len);
        String::from_utf16(slice).ok()
    }
}

/// Start audio capture (stub - not implemented).
///
/// Audio capture will be implemented in a future change (add-windows-audio-capture).
pub fn start_audio_capture(_source_id: &str) -> Result<(AudioReceiver, StopHandle), CaptureError> {
    Err(CaptureError::NotImplemented(
        "Audio capture not yet implemented for Windows".to_string(),
    ))
}

/// Initialize the audio backend.
///
/// For Windows, no persistent backend is needed for enumeration-only support.
/// This function succeeds silently to allow the application to start.
pub fn init_audio_backend() -> Result<(), String> {
    eprintln!("[Audio] Windows audio enumeration initialized (capture not yet implemented)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_list_audio_sources_returns_devices() {
        // This test verifies that on a typical Windows system with audio hardware,
        // we can enumerate devices. The test passes even with 0 devices.
        let result = list_audio_sources();
        assert!(result.is_ok());

        let devices = result.unwrap();
        // Print devices for manual verification (use cargo test -- --nocapture)
        eprintln!("\n=== Audio Device Enumeration Test ===");
        eprintln!("Found {} audio devices:", devices.len());
        for device in &devices {
            eprintln!(
                "  - {} [{}] (type: {:?})",
                device.name, device.id, device.source_type
            );
        }
        eprintln!("=====================================\n");

        // Verify device properties if any devices exist
        for device in &devices {
            // ID should not be empty
            assert!(!device.id.is_empty(), "Device ID should not be empty");
            // Name should not be empty
            assert!(!device.name.is_empty(), "Device name should not be empty");
        }
    }
}
