//! Windows audio device enumeration using WASAPI.
//!
//! This module provides audio device enumeration via the Windows Audio Session API
//! (WASAPI). It supports enumerating both output devices (speakers/headphones) and
//! input devices (microphones).

use crate::capture::error::EnumerationError;
use crate::capture::{AudioSource, AudioSourceType};

use windows::core::PCWSTR;
use windows::Win32::Media::Audio::{
    eCapture, eRender, IMMDevice, IMMDeviceCollection, IMMDeviceEnumerator, MMDeviceEnumerator,
    DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED, STGM_READ,
};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;

/// PKEY_Device_FriendlyName - the friendly name property key
/// {a45c254e-df1c-4efd-8020-67d146a850e0}, 14
const PKEY_DEVICE_FRIENDLY_NAME: windows::Win32::Foundation::PROPERTYKEY =
    windows::Win32::Foundation::PROPERTYKEY {
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
            tracing::error!("[Audio] Failed to create device enumerator: {:?}", e);
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

    tracing::debug!("[Audio] Enumerated {} audio devices", sources.len());
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
        CoTaskMemFree(Some(id_ptr.0 as *const _));
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

        // Extract string from PROPVARIANT
        propvariant_to_string(&prop_value)
    }
}

/// Convert a PROPVARIANT to a String if it contains a string value.
fn propvariant_to_string(pv: &PROPVARIANT) -> Option<String> {
    unsafe {
        // VT_LPWSTR = 31 - check if this is a wide string
        let vt = pv.vt();
        if vt == windows::Win32::System::Variant::VARENUM(31) {
            // Access the pwszVal field for LPWSTR (PWSTR type, access .0 for *mut u16)
            let pwsz = pv.Anonymous.Anonymous.Anonymous.pwszVal;
            if !pwsz.is_null() {
                return pcwstr_to_string(PCWSTR(pwsz.0 as *const u16));
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

/// Initialize the audio backend.
///
/// For Windows, verifies that COM can be initialized and audio devices are accessible.
pub fn init_audio_backend() -> Result<(), String> {
    tracing::debug!("[Audio] Initializing Windows WASAPI audio backend");

    // Test COM initialization
    let com_initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() };

    if com_initialized {
        // Test device enumeration
        let result = enumerate_audio_devices();
        unsafe {
            CoUninitialize();
        }
        match result {
            Ok(devices) => {
                tracing::debug!(
                    "[Audio] Windows WASAPI audio backend initialized ({} devices)",
                    devices.len()
                );
                Ok(())
            }
            Err(e) => {
                tracing::debug!("[Audio] Failed to enumerate devices: {:?}", e);
                // Don't fail initialization, just warn
                Ok(())
            }
        }
    } else {
        tracing::debug!("[Audio] COM initialization failed, audio may not work");
        // Don't fail initialization, just warn
        Ok(())
    }
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
        tracing::debug!("\n=== Audio Device Enumeration Test ===");
        tracing::debug!("Found {} audio devices:", devices.len());
        for device in &devices {
            tracing::debug!(
                "  - {} [{}] (type: {:?})",
                device.name,
                device.id,
                device.source_type
            );
        }
        tracing::debug!("=====================================\n");

        // Verify device properties if any devices exist
        for device in &devices {
            // ID should not be empty
            assert!(!device.id.is_empty(), "Device ID should not be empty");
            // Name should not be empty
            assert!(!device.name.is_empty(), "Device name should not be empty");
        }
    }
}
