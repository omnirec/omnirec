//! macOS-specific platform functionality.
//!
//! This module provides minimal macOS-specific checks that need to run in the Tauri client,
//! such as permission checks and version detection.

// Core Graphics FFI for permission checks
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

/// Check if screen recording permission is granted.
pub fn has_screen_recording_permission() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() }
}

/// Trigger the permission prompt to add the app to the Screen Recording list.
///
/// This calls CGRequestScreenCaptureAccess which will cause macOS to
/// show the permission prompt (first time only) and add the app to the
/// Screen Recording list in System Settings.
pub fn trigger_permission_prompt() {
    unsafe { CGRequestScreenCaptureAccess() };
}

/// Check if system audio capture is available on this macOS version.
///
/// ScreenCaptureKit audio capture requires macOS 13+.
pub fn is_system_audio_available() -> bool {
    is_macos_13_or_later()
}

/// Check if running on macOS 13 or later.
fn is_macos_13_or_later() -> bool {
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
