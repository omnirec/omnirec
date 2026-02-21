//! Capture and thumbnail commands.
//!
//! Commands for listing windows/monitors, capturing thumbnails, and showing highlights.
//! These commands interact directly with the capture backends.

use crate::capture::{self, ThumbnailCapture};
use crate::AppState;
use omnirec_common::{MonitorInfo, WindowInfo};
use tauri::State;

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

/// Get list of capturable windows.
#[tauri::command]
pub async fn get_windows(_state: State<'_, AppState>) -> Result<Vec<WindowInfo>, String> {
    Ok(capture::list_windows())
}

/// Get list of available monitors.
#[tauri::command]
pub async fn get_monitors(_state: State<'_, AppState>) -> Result<Vec<MonitorInfo>, String> {
    Ok(capture::list_monitors())
}

/// Show a highlight border on the specified monitor.
#[tauri::command]
pub async fn show_display_highlight(
    monitor_id: String,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    let monitors = capture::list_monitors();
    let monitor = monitors
        .iter()
        .find(|m| m.id == monitor_id)
        .ok_or_else(|| format!("Monitor not found: {}", monitor_id))?;

    capture::show_highlight(monitor.x, monitor.y, monitor.width as i32, monitor.height as i32);
    Ok(())
}

/// Show a highlight border on the specified window.
#[tauri::command]
pub async fn show_window_highlight(
    window_handle: isize,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    let windows = capture::list_windows();
    let window = windows
        .iter()
        .find(|w| w.handle == window_handle)
        .ok_or_else(|| format!("Window not found: {}", window_handle))?;

    capture::show_highlight(window.x, window.y, window.width as i32, window.height as i32);
    Ok(())
}

/// Capture a thumbnail of a window.
///
/// Returns a base64-encoded JPEG image or null if capture fails.
#[tauri::command]
pub async fn get_window_thumbnail(
    window_handle: isize,
    _state: State<'_, AppState>,
) -> Result<Option<ThumbnailResponse>, String> {
    let backend = capture::get_backend();
    match backend.capture_window_thumbnail(window_handle) {
        Ok(result) => Ok(Some(ThumbnailResponse {
            data: result.data,
            width: result.width,
            height: result.height,
        })),
        Err(e) => {
            tracing::warn!("Window thumbnail capture failed: {}", e);
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
    _state: State<'_, AppState>,
) -> Result<Option<ThumbnailResponse>, String> {
    let backend = capture::get_backend();
    match backend.capture_display_thumbnail(&monitor_id) {
        Ok(result) => Ok(Some(ThumbnailResponse {
            data: result.data,
            width: result.width,
            height: result.height,
        })),
        Err(e) => {
            tracing::warn!("Display thumbnail capture failed: {}", e);
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
    _state: State<'_, AppState>,
) -> Result<Option<ThumbnailResponse>, String> {
    let backend = capture::get_backend();
    match backend.capture_region_preview(&monitor_id, x, y, width, height) {
        Ok(result) => Ok(Some(ThumbnailResponse {
            data: result.data,
            width: result.width,
            height: result.height,
        })),
        Err(e) => {
            tracing::warn!("Region preview capture failed: {}", e);
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
        use crate::platform::macos;

        // First check if we already have permission
        if macos::has_screen_recording_permission() {
            return "granted".to_string();
        }

        // If not granted, trigger the prompt to add app to the permission list
        // This causes macOS to show the permission dialog (first time only)
        // and adds the app to System Settings > Screen Recording
        macos::trigger_permission_prompt();

        // Check again after triggering
        if macos::has_screen_recording_permission() {
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
        use crate::platform::macos;
        macos::trigger_permission_prompt();

        // Then open System Settings directly to the Screen Recording pane
        // This URL scheme works on macOS 13+ (Ventura and later)
        // Falls back to Privacy & Security on older versions
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .spawn();
    }
}