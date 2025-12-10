//! macOS window enumeration using ScreenCaptureKit.
//!
//! Uses SCShareableContent for window enumeration, which provides accurate
//! window information once screen recording permission is granted.

use crate::capture::types::WindowInfo;
use screencapturekit::sc_shareable_content::SCShareableContent;

/// List all visible, capturable windows on macOS.
///
/// Uses ScreenCaptureKit's SCShareableContent for accurate window enumeration.
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

        windows.push(WindowInfo {
            handle: window.window_id as isize,
            title,
            process_name,
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
