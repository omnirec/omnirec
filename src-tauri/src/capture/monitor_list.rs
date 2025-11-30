//! Monitor enumeration using Windows API.

use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
    DISPLAY_DEVICEW, DISPLAY_DEVICE_ACTIVE,
};

/// Information about a display monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    /// Unique identifier (device name)
    pub id: String,
    /// Display name for UI
    pub name: String,
    /// Virtual screen X position
    pub x: i32,
    /// Virtual screen Y position
    pub y: i32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Whether this is the primary monitor
    pub is_primary: bool,
}

/// List all connected monitors.
pub fn list_monitors() -> Vec<MonitorInfo> {
    let mut monitors: Vec<MonitorInfo> = Vec::new();

    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(enum_monitor_callback),
            LPARAM(&mut monitors as *mut Vec<MonitorInfo> as isize),
        );
    }

    // Sort so primary is first
    monitors.sort_by(|a, b| b.is_primary.cmp(&a.is_primary));

    monitors
}

/// Callback for EnumDisplayMonitors.
unsafe extern "system" fn enum_monitor_callback(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let monitors = &mut *(lparam.0 as *mut Vec<MonitorInfo>);

    // Get monitor info
    let mut monitor_info = MONITORINFOEXW::default();
    monitor_info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    if GetMonitorInfoW(hmonitor, &mut monitor_info as *mut _ as *mut _).as_bool() {
        let rect = monitor_info.monitorInfo.rcMonitor;
        let is_primary = (monitor_info.monitorInfo.dwFlags & 1) != 0; // MONITORINFOF_PRIMARY = 1

        // Get device name
        let device_name_raw = &monitor_info.szDevice;
        let device_name_len = device_name_raw
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(device_name_raw.len());
        let device_name = OsString::from_wide(&device_name_raw[..device_name_len])
            .to_string_lossy()
            .to_string();

        // Get friendly display name
        let display_name = get_display_friendly_name(&device_name)
            .unwrap_or_else(|| format_monitor_name(&device_name, is_primary));

        monitors.push(MonitorInfo {
            id: device_name,
            name: display_name,
            x: rect.left,
            y: rect.top,
            width: (rect.right - rect.left) as u32,
            height: (rect.bottom - rect.top) as u32,
            is_primary,
        });
    }

    BOOL(1) // Continue enumeration
}

/// Get friendly display name from device name.
fn get_display_friendly_name(device_name: &str) -> Option<String> {
    unsafe {
        let mut device = DISPLAY_DEVICEW::default();
        device.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;

        let mut index = 0u32;
        while EnumDisplayDevicesW(None, index, &mut device, 0).as_bool() {
            let current_name_len = device.DeviceName
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(device.DeviceName.len());
            let current_name = OsString::from_wide(&device.DeviceName[..current_name_len])
                .to_string_lossy()
                .to_string();

            if current_name == device_name && (device.StateFlags & DISPLAY_DEVICE_ACTIVE) != 0 {
                // Get adapter info
                let adapter_name_len = device.DeviceString
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(device.DeviceString.len());
                if adapter_name_len > 0 {
                    return Some(
                        OsString::from_wide(&device.DeviceString[..adapter_name_len])
                            .to_string_lossy()
                            .to_string()
                    );
                }
            }
            index += 1;
        }
    }
    None
}

/// Format a basic monitor name.
fn format_monitor_name(device_name: &str, is_primary: bool) -> String {
    let suffix = if is_primary { " (Primary)" } else { "" };
    // Extract display number from device name like "\\\\.\\DISPLAY1"
    if let Some(num) = device_name.chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse::<u32>().ok() {
        format!("Display {}{}", num, suffix)
    } else {
        format!("{}{}", device_name, suffix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_monitors_returns_at_least_one() {
        let monitors = list_monitors();
        assert!(!monitors.is_empty(), "Should have at least one monitor");
    }

    #[test]
    fn test_primary_monitor_first() {
        let monitors = list_monitors();
        if !monitors.is_empty() {
            // Primary should be first if it exists
            let has_primary = monitors.iter().any(|m| m.is_primary);
            if has_primary {
                assert!(monitors[0].is_primary, "Primary monitor should be first");
            }
        }
    }

    #[test]
    fn test_monitor_dimensions_valid() {
        let monitors = list_monitors();
        for monitor in monitors {
            assert!(monitor.width > 0, "Monitor width should be positive");
            assert!(monitor.height > 0, "Monitor height should be positive");
        }
    }
}
