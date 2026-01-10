//! Monitor enumeration using Windows API.

use crate::capture::MonitorInfo;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, DISPLAY_DEVICEW,
    DISPLAY_DEVICE_ACTIVE, HDC, HMONITOR, MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

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

        // Get DPI scale factor for this monitor (still useful for UI scaling)
        let scale_factor = get_monitor_scale_factor(hmonitor);

        // With Per-Monitor DPI Aware v2 (set in main.rs), Windows returns physical
        // pixel coordinates in the virtual screen coordinate space. These match
        // the coordinates that Tauri's window.outerPosition() returns.
        //
        // We return physical coordinates directly - no conversion needed.
        // The frontend should also use physical coordinates from Tauri directly.
        // This keeps everything in a consistent coordinate space.
        let physical_x = rect.left;
        let physical_y = rect.top;
        let physical_width = (rect.right - rect.left) as u32;
        let physical_height = (rect.bottom - rect.top) as u32;

        monitors.push(MonitorInfo {
            id: device_name,
            name: display_name,
            x: physical_x,
            y: physical_y,
            width: physical_width,
            height: physical_height,
            is_primary,
            scale_factor,
        });
    }

    BOOL(1) // Continue enumeration
}

/// Get the DPI scale factor for a monitor.
/// Returns 1.0 if DPI cannot be determined.
fn get_monitor_scale_factor(hmonitor: HMONITOR) -> f64 {
    const DEFAULT_DPI: u32 = 96; // Windows baseline DPI (100% scaling)

    unsafe {
        let mut dpi_x: u32 = 0;
        let mut dpi_y: u32 = 0;

        if GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok() {
            // Use horizontal DPI for scale factor calculation
            dpi_x as f64 / DEFAULT_DPI as f64
        } else {
            1.0 // Fallback to 100% scaling if API fails
        }
    }
}

/// Get friendly display name from device name.
fn get_display_friendly_name(device_name: &str) -> Option<String> {
    unsafe {
        let mut device = DISPLAY_DEVICEW {
            cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };

        let mut index = 0u32;
        while EnumDisplayDevicesW(None, index, &mut device, 0).as_bool() {
            let current_name_len = device
                .DeviceName
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(device.DeviceName.len());
            let current_name = OsString::from_wide(&device.DeviceName[..current_name_len])
                .to_string_lossy()
                .to_string();

            if current_name == device_name && (device.StateFlags & DISPLAY_DEVICE_ACTIVE) != 0 {
                // Get adapter info
                let adapter_name_len = device
                    .DeviceString
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(device.DeviceString.len());
                if adapter_name_len > 0 {
                    return Some(
                        OsString::from_wide(&device.DeviceString[..adapter_name_len])
                            .to_string_lossy()
                            .to_string(),
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
    if let Ok(num) = device_name
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse::<u32>()
    {
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

    #[test]
    fn test_monitor_scale_factor_valid() {
        let monitors = list_monitors();
        for monitor in &monitors {
            // Scale factor should be at least 1.0 (100%)
            assert!(
                monitor.scale_factor >= 1.0,
                "Monitor {} scale factor {} should be >= 1.0",
                monitor.name,
                monitor.scale_factor
            );
            // Scale factor shouldn't exceed 5.0 (500%) - reasonable upper bound
            assert!(
                monitor.scale_factor <= 5.0,
                "Monitor {} scale factor {} should be <= 5.0",
                monitor.name,
                monitor.scale_factor
            );
        }
        // Print full monitor info for debugging
        // Note: width/height are now physical pixels (no conversion needed)
        println!("\n=== MONITOR INFO DEBUG (Physical Coordinates) ===");
        for monitor in &monitors {
            let logical_width = (monitor.width as f64 / monitor.scale_factor).round() as u32;
            let logical_height = (monitor.height as f64 / monitor.scale_factor).round() as u32;
            println!(
                "Monitor '{}' (id={}):\n  Physical Position: ({}, {})\n  Physical Size: {}x{}\n  Logical size: {}x{}\n  Scale: {:.0}% (factor: {})\n  Primary: {}",
                monitor.name,
                monitor.id,
                monitor.x,
                monitor.y,
                monitor.width,
                monitor.height,
                logical_width,
                logical_height,
                monitor.scale_factor * 100.0,
                monitor.scale_factor,
                monitor.is_primary
            );
        }
        println!("=================================================\n");
    }
}
