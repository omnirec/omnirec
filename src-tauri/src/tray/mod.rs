//! Cross-platform system tray functionality.
//!
//! This module provides a unified tray interface across all platforms:
//! - Linux: Full implementation with tray icon and menu
//! - Windows: Full implementation with tray icon and menu
//! - macOS: Full implementation with menu bar icon and menu
//!
//! On Linux portal-mode desktops (GNOME, KDE, COSMIC), the tray provides
//! the primary interface for recording controls.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
mod windows;

// Platform-specific implementations are accessed through the unified functions below.
// The submodules are kept private; only the cross-platform API is exposed.
// Exception: macOS module is public for menu event handling from lib.rs.

// =============================================================================
// Shared Types
// =============================================================================

/// Menu item identifiers used across all platforms.
pub mod menu_ids {
    pub const RECORD: &str = "record";
    pub const STOP: &str = "stop";
    pub const TRANSCRIPTION: &str = "transcription";
    pub const ALWAYS_ON_TOP: &str = "always_on_top";
    pub const CONFIGURATION: &str = "configuration";
    pub const LOGS: &str = "logs";
    pub const ABOUT: &str = "about";
    pub const EXIT: &str = "exit";
}

/// Menu item labels used across all platforms.
pub mod menu_labels {
    pub const RECORD: &str = "Record Screen/Window";
    pub const STOP: &str = "Stop Recording";
    pub const TRANSCRIPTION: &str = "Transcription";
    pub const ALWAYS_ON_TOP: &str = "Always on Top";
    pub const CONFIGURATION: &str = "Configuration";
    pub const LOGS: &str = "Logs";
    pub const ABOUT: &str = "About";
    pub const EXIT: &str = "Exit";
}

/// Tray icon file names.
pub mod icon_names {
    /// Symbolic (monochrome) icons for GNOME/KDE.
    pub const SYMBOLIC_22: &str = "omnirec-symbolic-22.png";
    pub const SYMBOLIC_24: &str = "omnirec-symbolic-24.png";
    pub const SYMBOLIC_32: &str = "omnirec-symbolic-32.png";
    pub const SYMBOLIC: &str = "omnirec-symbolic.png";

    /// Full-color icons for COSMIC and other platforms.
    pub const COLOR_128: &str = "128x128.png";
    pub const COLOR_64: &str = "64x64.png";
    pub const COLOR_32: &str = "32x32.png";

    /// Recording indicator icons.
    pub const RECORDING_22: &str = "omnirec-recording-22.png";
    pub const RECORDING_24: &str = "omnirec-recording-24.png";
    pub const RECORDING_32: &str = "omnirec-recording-32.png";
    pub const RECORDING: &str = "omnirec-recording.png";

    /// macOS template icons (monochrome, adapts to menu bar appearance).
    pub const TEMPLATE: &str = "omnirec-template.png";
    pub const TEMPLATE_2X: &str = "omnirec-template@2x.png";
    pub const RECORDING_TEMPLATE: &str = "omnirec-recording-template.png";
    pub const RECORDING_TEMPLATE_2X: &str = "omnirec-recording-template@2x.png";
}

// =============================================================================
// Cross-Platform Tray State
// =============================================================================

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::menu::CheckMenuItem;
use tauri::tray::TrayIcon;

// =============================================================================
// Shared Window Helpers
// =============================================================================

/// Open the config window directly from Rust, without relying on a frontend
/// event listener that may not fire when the main window is hidden.
///
/// If the window already exists it is shown and focused; otherwise it is
/// created with the same properties used by the TypeScript `openConfigWindow`.
pub fn open_config_window(app: &tauri::AppHandle) {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    if let Some(w) = app.get_webview_window("config") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }

    match WebviewWindowBuilder::new(app, "config", WebviewUrl::App("src/config.html".into()))
        .title("OmniRec Settings")
        .inner_size(450.0, 550.0)
        .resizable(false)
        .maximizable(false)
        .decorations(false)
        .shadow(true)
        .build()
    {
        Ok(_) => eprintln!("[Tray] Config window created"),
        Err(e) => eprintln!("[Tray] Failed to create config window: {:?}", e),
    }
}

/// Open the about window directly from Rust, without relying on a frontend
/// event listener that may not fire when the main window is hidden.
///
/// If the window already exists it is shown and focused; otherwise it is
/// created with the same properties used by the TypeScript `openAboutWindow`.
pub fn open_about_window(app: &tauri::AppHandle) {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    if let Some(w) = app.get_webview_window("about") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }

    match WebviewWindowBuilder::new(app, "about", WebviewUrl::App("src/about.html".into()))
        .title("About OmniRec")
        .inner_size(350.0, 400.0)
        .resizable(false)
        .maximizable(false)
        .decorations(false)
        .shadow(true)
        .build()
    {
        Ok(_) => eprintln!("[Tray] About window created"),
        Err(e) => eprintln!("[Tray] Failed to create about window: {:?}", e),
    }
}

/// Open the log viewer window, or focus it if already open.
pub fn open_log_viewer_window(app: &tauri::AppHandle) {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    if let Some(w) = app.get_webview_window("logs") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }

    match WebviewWindowBuilder::new(app, "logs", WebviewUrl::App("logs.html".into()))
        .title("OmniRec Logs")
        .inner_size(900.0, 600.0)
        .min_inner_size(600.0, 400.0)
        .resizable(true)
        .decorations(false)
        .shadow(true)
        .skip_taskbar(true)
        .center()
        .build()
    {
        Ok(_) => eprintln!("[Tray] Logs window created"),
        Err(e) => eprintln!("[Tray] Failed to create logs window: {:?}", e),
    }
}

/// System tray state.
///
/// This struct holds the tray icon handle and recording state.
pub struct TrayState {
    /// The tray icon handle.
    pub tray: std::sync::Mutex<TrayIcon>,

    /// Whether a recording is currently in progress.
    pub is_recording: Arc<AtomicBool>,

    /// The "Always on Top" check menu item handle (for toggling the checkmark).
    pub always_on_top_item: std::sync::Mutex<Option<CheckMenuItem<tauri::Wry>>>,
}

/// Toggle the always-on-top window property and persist the new state.
///
/// Reads the current state from config, flips it, applies it to the main
/// window, updates the tray menu checkmark, and writes the new value to disk.
pub fn toggle_always_on_top(app: &tauri::AppHandle) {
    use tauri::Manager;

    // Load current config, flip the flag, save
    let mut config = crate::config::load_config();
    config.always_on_top = !config.always_on_top;
    let new_state = config.always_on_top;

    if let Err(e) = crate::config::save_config(&config) {
        eprintln!("[Tray] Failed to save always-on-top config: {}", e);
    }

    // Apply to the main window
    if let Some(window) = app.get_webview_window("main") {
        if let Err(e) = window.set_always_on_top(new_state) {
            eprintln!("[Tray] Failed to set always-on-top: {:?}", e);
        }
    }

    // Sync the in-memory AppState config so other code sees the update
    if let Some(state) = app.try_state::<crate::AppState>() {
        let config_clone = config.clone();
        let state_config = state.app_config.clone();
        tauri::async_runtime::spawn(async move {
            let mut guard = state_config.lock().await;
            *guard = config_clone;
        });
    }

    // Update the checkmark on the tray menu item
    if let Some(tray_state) = app.try_state::<TrayState>() {
        if let Ok(guard) = tray_state.always_on_top_item.lock() {
            if let Some(item) = guard.as_ref() {
                if let Err(e) = item.set_checked(new_state) {
                    eprintln!("[Tray] Failed to update always-on-top checkmark: {:?}", e);
                }
            }
        }
    }

    eprintln!("[Tray] Always on Top toggled to: {}", new_state);
}

// =============================================================================
// Cross-Platform Functions (with platform-specific implementations)
// =============================================================================

/// Set up the system tray.
///
/// This is called during application startup on all platforms.
/// On Linux portal-mode desktops, this creates the tray icon and hides the main window.
/// On other platforms, this is currently a stub.
#[cfg(target_os = "linux")]
pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    linux::setup_tray(app)
}

#[cfg(target_os = "windows")]
pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    windows::setup_tray(app)
}

#[cfg(target_os = "macos")]
pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    macos::setup_tray(app)
}

/// Set tray icon visibility.
///
/// On Linux, hides the tray during recording (GNOME's system indicator is used).
/// On other platforms, this is currently a no-op.
#[cfg(target_os = "linux")]
pub fn set_tray_visible(app: &tauri::AppHandle, visible: bool) {
    linux::set_tray_visible(app, visible)
}

#[cfg(target_os = "windows")]
pub fn set_tray_visible(app: &tauri::AppHandle, visible: bool) {
    windows::set_tray_visible(app, visible)
}

#[cfg(target_os = "macos")]
pub fn set_tray_visible(app: &tauri::AppHandle, visible: bool) {
    macos::set_tray_visible(app, visible)
}

/// Update the tray icon to reflect recording state.
///
/// When recording, the icon changes to a red indicator.
/// When idle, the normal application icon is shown.
/// This also updates the menu item enabled states.
#[cfg(target_os = "windows")]
pub fn set_recording_state(app: &tauri::AppHandle, recording: bool) {
    windows::set_recording_state(app, recording)
}

#[cfg(target_os = "macos")]
pub fn set_recording_state(app: &tauri::AppHandle, recording: bool) {
    macos::set_recording_state(app, recording)
}

#[cfg(target_os = "linux")]
pub fn set_recording_state(_app: &tauri::AppHandle, _recording: bool) {
    // No-op on Linux - GNOME's system indicator is used during recording
}

/// Check if running in portal mode.
///
/// Portal mode is active on Linux desktops (GNOME, KDE, COSMIC) that use
/// the native portal picker for source selection. In portal mode:
/// - The main window is hidden on startup
/// - Recording is initiated from the tray menu
/// - The portal's native picker handles source selection
///
/// On non-Linux platforms, this always returns false.
#[cfg(target_os = "linux")]
pub fn is_portal_mode() -> bool {
    linux::is_portal_mode()
}

#[cfg(not(target_os = "linux"))]
pub fn is_portal_mode() -> bool {
    false
}

// =============================================================================
// Legacy Aliases (for backwards compatibility during refactor)
// =============================================================================

/// Legacy alias for `TrayState`.
/// TODO: Remove after all code is updated to use `TrayState`.
pub type GnomeTrayState = TrayState;

/// Legacy alias for `setup_tray`.
/// TODO: Remove after all code is updated to use `setup_tray`.
pub fn setup_tray_mode(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    setup_tray(app)
}

/// Legacy alias for `set_tray_visible`.
/// TODO: Remove after all code is updated to use `set_tray_visible`.
pub fn set_gnome_tray_visible(app: &tauri::AppHandle, visible: bool) {
    set_tray_visible(app, visible)
}

/// Legacy alias for `is_portal_mode`.
/// TODO: Remove after all code is updated to use `is_portal_mode`.
#[cfg(target_os = "linux")]
pub fn is_tray_mode_desktop() -> bool {
    is_portal_mode()
}
