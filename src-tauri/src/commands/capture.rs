//! Capture and thumbnail commands.
//!
//! Commands for listing windows/monitors, capturing thumbnails, and showing highlights.

use crate::capture::{
    get_backend, list_monitors, list_windows, show_highlight, MonitorInfo, ThumbnailCapture,
    ThumbnailResult, WindowInfo,
};

/// Thumbnail result for JSON serialization.
#[derive(serde::Serialize)]
pub struct ThumbnailResponse {
    /// Base64-encoded JPEG image data
    pub data: String,
    /// Thumbnail width in pixels
    pub width: u32,
    /// Thumbnail height in pixels
    pub height: u32,
}

impl From<ThumbnailResult> for ThumbnailResponse {
    fn from(result: ThumbnailResult) -> Self {
        Self {
            data: result.data,
            width: result.width,
            height: result.height,
        }
    }
}

/// Get list of capturable windows.
#[tauri::command]
pub fn get_windows() -> Vec<WindowInfo> {
    list_windows()
}

/// Get list of available monitors.
#[tauri::command]
pub fn get_monitors() -> Vec<MonitorInfo> {
    list_monitors()
}

/// Show a highlight border on the specified monitor.
#[tauri::command]
pub async fn show_display_highlight(monitor_id: String) -> Result<(), String> {
    // Find the monitor
    let monitors = list_monitors();
    let monitor = monitors
        .iter()
        .find(|m| m.id == monitor_id)
        .ok_or_else(|| format!("Monitor not found: {}", monitor_id))?;

    show_highlight(
        monitor.x,
        monitor.y,
        monitor.width as i32,
        monitor.height as i32,
    );

    Ok(())
}

/// Show a highlight border on the specified window.
#[tauri::command]
pub async fn show_window_highlight(window_handle: isize) -> Result<(), String> {
    // Find the window
    let windows = list_windows();
    let window = windows
        .iter()
        .find(|w| w.handle == window_handle)
        .ok_or_else(|| format!("Window not found: {}", window_handle))?;

    // Only show highlight if window has valid dimensions
    if window.width > 0 && window.height > 0 {
        show_highlight(
            window.x,
            window.y,
            window.width as i32,
            window.height as i32,
        );
    }

    Ok(())
}

/// Capture a thumbnail of a window.
///
/// Returns a base64-encoded JPEG image or null if capture fails.
#[tauri::command]
pub async fn get_window_thumbnail(
    window_handle: isize,
) -> Result<Option<ThumbnailResponse>, String> {
    let backend = get_backend();
    match backend.capture_window_thumbnail(window_handle) {
        Ok(result) => Ok(Some(result.into())),
        Err(_) => {
            // Fail gracefully - show placeholder for any error
            Ok(None)
        }
    }
}

/// Capture a thumbnail of a display.
///
/// Returns a base64-encoded JPEG image or null if capture fails.
#[tauri::command]
pub async fn get_display_thumbnail(
    monitor_id: String,
) -> Result<Option<ThumbnailResponse>, String> {
    let backend = get_backend();
    match backend.capture_display_thumbnail(&monitor_id) {
        Ok(result) => Ok(Some(result.into())),
        Err(e) => {
            eprintln!("[get_display_thumbnail] Error: {}", e);
            // Fail gracefully - show placeholder for any error
            Ok(None)
        }
    }
}

/// Capture a preview of a screen region.
///
/// Returns a base64-encoded JPEG image or null if capture fails.
#[tauri::command]
pub async fn get_region_preview(
    monitor_id: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<Option<ThumbnailResponse>, String> {
    let backend = get_backend();
    match backend.capture_region_preview(&monitor_id, x, y, width, height) {
        Ok(result) => Ok(Some(result.into())),
        Err(e) => {
            eprintln!("[get_region_preview] Error: {}", e);
            // Fail gracefully - show placeholder for any error
            Ok(None)
        }
    }
}

/// Check screen recording permission status (macOS only).
/// Returns: "granted", "denied", or "unknown" (non-macOS platforms).
///
/// This also triggers the permission prompt on first run to ensure
/// the app appears in the Screen Recording permission list.
#[tauri::command]
pub fn check_screen_recording_permission() -> String {
    #[cfg(target_os = "macos")]
    {
        use crate::capture::macos::MacOSBackend;

        // First check if we already have permission
        if MacOSBackend::has_screen_recording_permission() {
            return "granted".to_string();
        }

        // If not granted, trigger the prompt to add app to the permission list
        // This causes macOS to show the permission dialog (first time only)
        // and adds the app to System Settings > Screen Recording
        MacOSBackend::trigger_permission_prompt();

        // Check again after triggering
        if MacOSBackend::has_screen_recording_permission() {
            "granted".to_string()
        } else {
            "denied".to_string()
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        "unknown".to_string()
    }
}

/// Open System Settings to the Screen Recording privacy pane (macOS only).
#[tauri::command]
pub fn open_screen_recording_settings() {
    #[cfg(target_os = "macos")]
    {
        // First trigger the permission prompt to ensure the app is in the list
        use crate::capture::macos::MacOSBackend;
        MacOSBackend::trigger_permission_prompt();

        // Then open System Settings directly to the Screen Recording pane
        // This URL scheme works on macOS 13+ (Ventura and later)
        // Falls back to Privacy & Security on older versions
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .spawn();
    }
}
