//! macOS window enumeration using ScreenCaptureKit.
//!
//! Uses SCShareableContent for window enumeration, which provides accurate
//! window information once screen recording permission is granted.
//! Window bounds are obtained via Core Graphics CGWindowListCopyWindowInfo
//! since the screencapturekit crate doesn't expose position data.

use crate::capture::WindowInfo;
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionaryRef;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::display::{
    kCGWindowListOptionIncludingWindow, CGDisplay, CGWindowID,
};
use screencapturekit::sc_shareable_content::SCShareableContent;
use std::collections::HashMap;

// External function for raw array access
extern "C" {
    fn CFArrayGetValueAtIndex(
        theArray: core_foundation::array::CFArrayRef,
        idx: isize,
    ) -> *const std::ffi::c_void;
}

/// Window bounds from Core Graphics
#[derive(Debug, Clone, Copy, Default)]
struct WindowBounds {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

/// Get window bounds for a specific window ID using Core Graphics.
///
/// This uses CGWindowListCopyWindowInfo to get the kCGWindowBounds dictionary
/// which contains X, Y, Width, Height of the window frame.
fn get_window_bounds(window_id: CGWindowID) -> Option<WindowBounds> {
    // Get window info for this specific window
    let info_array = CGDisplay::window_list_info(
        kCGWindowListOptionIncludingWindow,
        Some(window_id),
    )?;

    if info_array.is_empty() {
        return None;
    }

    // Get the first (and only) window info dictionary using raw CFArray access
    let dict_ref: CFDictionaryRef = unsafe {
        let ptr = CFArrayGetValueAtIndex(info_array.as_concrete_TypeRef(), 0);
        if ptr.is_null() {
            return None;
        }
        ptr as CFDictionaryRef
    };

    // Keys for window bounds dictionary
    let bounds_key = CFString::new("kCGWindowBounds");

    // Get the bounds dictionary
    let bounds_value = unsafe {
        let mut value: *const std::ffi::c_void = std::ptr::null();
        if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
            dict_ref,
            bounds_key.as_concrete_TypeRef() as *const _,
            &mut value,
        ) == 0
        {
            return None;
        }
        value as CFDictionaryRef
    };

    // Extract X, Y, Width, Height from bounds dictionary
    let x = get_number_from_dict(bounds_value, "X").unwrap_or(0.0) as i32;
    let y = get_number_from_dict(bounds_value, "Y").unwrap_or(0.0) as i32;
    let width = get_number_from_dict(bounds_value, "Width").unwrap_or(0.0) as u32;
    let height = get_number_from_dict(bounds_value, "Height").unwrap_or(0.0) as u32;

    Some(WindowBounds { x, y, width, height })
}

/// Helper to extract a number from a CFDictionary
fn get_number_from_dict(dict: CFDictionaryRef, key: &str) -> Option<f64> {
    let cf_key = CFString::new(key);
    unsafe {
        let mut value: *const std::ffi::c_void = std::ptr::null();
        if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
            dict,
            cf_key.as_concrete_TypeRef() as *const _,
            &mut value,
        ) == 0
        {
            return None;
        }
        let cf_number = CFNumber::wrap_under_get_rule(value as _);
        cf_number.to_f64()
    }
}

/// Build a map of window ID to bounds for efficient lookup
fn build_window_bounds_map(window_ids: &[CGWindowID]) -> HashMap<CGWindowID, WindowBounds> {
    let mut map = HashMap::new();
    for &window_id in window_ids {
        if let Some(bounds) = get_window_bounds(window_id) {
            map.insert(window_id, bounds);
        }
    }
    map
}

/// List all visible, capturable windows on macOS.
///
/// Uses ScreenCaptureKit's SCShareableContent for accurate window enumeration.
/// Window positions are obtained from Core Graphics since screencapturekit
/// doesn't expose position data.
///
/// This requires screen recording permission to return complete results.
///
/// Note: Without screen recording permission, this may return an empty list
/// or fail. The permission should be requested before calling this function.
pub fn list_windows() -> Vec<WindowInfo> {
    // Try to get shareable content - this requires screen recording permission
    let content = match SCShareableContent::try_current() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[macOS] Failed to get SCShareableContent: {}", e);
            eprintln!("[macOS] Screen recording permission may not be granted.");
            eprintln!("[macOS] Grant permission in System Settings > Privacy & Security > Screen Recording");
            return Vec::new();
        }
    };

    // Collect window IDs first to batch-fetch bounds
    let window_ids: Vec<CGWindowID> = content
        .windows
        .iter()
        .map(|w| w.window_id)
        .collect();

    // Get window bounds from Core Graphics
    let bounds_map = build_window_bounds_map(&window_ids);

    let mut windows = Vec::new();

    for window in &content.windows {
        // Skip windows without titles
        let title = match &window.title {
            Some(t) if !t.is_empty() => t.clone(),
            _ => continue,
        };

        // Skip windows that aren't on screen
        if !window.is_on_screen {
            continue;
        }

        // Skip windows that aren't at the normal layer
        // window_layer 0 = normal windows, other layers are system UI elements
        if window.window_layer != 0 {
            continue;
        }

        // Skip windows with zero or very small dimensions (likely invisible helper windows)
        if window.width < 50 || window.height < 50 {
            continue;
        }

        // Get application info
        let (process_name, bundle_id) = match &window.owning_application {
            Some(app) => (
                app.application_name.clone().unwrap_or_else(|| "Unknown".to_string()),
                app.bundle_identifier.clone().unwrap_or_default(),
            ),
            None => continue, // Skip windows without an owning application
        };

        // Filter out system components by bundle identifier (most reliable method)
        // These are macOS system processes that create windows but aren't user apps
        if bundle_id.starts_with("com.apple.dock")
            || bundle_id.starts_with("com.apple.controlcenter")
            || bundle_id.starts_with("com.apple.notificationcenterui")
            || bundle_id.starts_with("com.apple.systemuiserver")
            || bundle_id.starts_with("com.apple.Spotlight")
            || bundle_id.starts_with("com.apple.WindowManager")
            || bundle_id == "com.apple.finder" && title == "Desktop"
        {
            continue;
        }

        // Get bounds from Core Graphics, fallback to ScreenCaptureKit dimensions
        let bounds = bounds_map
            .get(&window.window_id)
            .copied()
            .unwrap_or(WindowBounds {
                x: 0,
                y: 0,
                width: window.width,
                height: window.height,
            });

        windows.push(WindowInfo {
            handle: window.window_id as isize,
            title,
            process_name,
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
        });
    }

    // Sort by process name, then by title for consistent ordering
    windows.sort_by(|a, b| {
        a.process_name
            .cmp(&b.process_name)
            .then_with(|| a.title.cmp(&b.title))
    });

    windows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_windows() {
        // This test requires screen recording permission to return meaningful results
        // In CI or without permission, it may return empty which is acceptable
        let windows = list_windows();
        
        // Just verify it doesn't crash and returns valid data if any
        for window in &windows {
            assert!(window.handle > 0, "Window handle should be > 0");
            assert!(!window.title.is_empty(), "Window title should not be empty");
        }
    }
}
