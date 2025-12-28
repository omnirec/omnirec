//! macOS system tray stub implementation.
//!
//! This module provides stub implementations for macOS.
//! Full tray support will be implemented in a future update.

/// Set up the system tray on macOS.
///
/// Currently a stub that returns success without creating a menu bar item.
/// Full implementation will be added in a future update.
pub fn setup_tray(_app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("[Tray] macOS tray stub: setup_tray called (no-op)");
    Ok(())
}

/// Set tray icon visibility on macOS.
///
/// Currently a no-op stub.
pub fn set_tray_visible(_app: &tauri::AppHandle, visible: bool) {
    eprintln!(
        "[Tray] macOS tray stub: set_tray_visible({}) called (no-op)",
        visible
    );
}
