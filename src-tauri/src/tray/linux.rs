//! Linux system tray implementation.
//!
//! This module provides full tray functionality for Linux desktops.
//! On portal-mode desktops (GNOME, KDE, COSMIC), the tray is the primary
//! interface for recording controls.

use super::{icon_names, menu_ids, menu_labels, TrayState};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::TrayIconBuilder,
    webview::WebviewUrl,
    Emitter, Manager,
};

/// Show or recreate the main window.
///
/// If the window exists, show and focus it. If it was destroyed (e.g., headless
/// mode), recreate it with the same configuration as the Tauri config.
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        // Window was destroyed (e.g., headless mode) — recreate it
        eprintln!("[Tray] Main window not found, recreating...");
        match tauri::WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
            .title("OmniRec")
            .inner_size(469.0, 610.0)
            .resizable(false)
            .maximizable(false)
            .decorations(false)
            .transparent(false)
            .shadow(true)
            .build()
        {
            Ok(window) => {
                eprintln!("[Tray] Main window recreated successfully");
                let _ = window.set_focus();
            }
            Err(e) => {
                eprintln!("[Tray] Failed to recreate main window: {}", e);
            }
        }
    }
}

// =============================================================================
// Portal Mode Detection
// =============================================================================

/// Check if running on a portal-mode desktop (GNOME, KDE, COSMIC).
///
/// These desktops use the portal's native picker for source selection,
/// so the app runs as a tray application without showing the main window.
///
/// Note: Cinnamon is NOT included because xdg-desktop-portal-xapp does not
/// implement ScreenCast.
pub fn is_portal_mode() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| {
            let upper = d.to_uppercase();
            upper.contains("GNOME") || upper.contains("KDE") || upper.contains("COSMIC")
        })
        .unwrap_or(false)
}

/// Check if running on COSMIC desktop (used for icon selection).
fn is_cosmic() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.to_uppercase().contains("COSMIC"))
        .unwrap_or(false)
}

// =============================================================================
// Icon Loading
// =============================================================================

/// Load a tray icon from multiple possible locations.
fn load_tray_icon(app: &tauri::App, icon_name: &str) -> Option<Image<'static>> {
    load_tray_icon_from_paths(app.path().resource_dir().ok(), icon_name)
}

/// Load a tray icon from multiple possible locations given a resource dir.
fn load_tray_icon_from_paths(
    resource_dir: Option<std::path::PathBuf>,
    icon_name: &str,
) -> Option<Image<'static>> {
    let resource_dir_clone = resource_dir.clone();
    let icon_paths = [
        // Production: resource_dir/icons/tray/
        resource_dir.map(|p| p.join(format!("icons/tray/{}", icon_name))),
        // Production: resource_dir/icons/ (for main app icons)
        resource_dir_clone.map(|p| p.join(format!("icons/{}", icon_name))),
        // Development: relative paths (tray subdirectory)
        Some(std::path::PathBuf::from(format!(
            "icons/tray/{}",
            icon_name
        ))),
        Some(std::path::PathBuf::from(format!(
            "src-tauri/icons/tray/{}",
            icon_name
        ))),
        // Development: relative paths (main icons directory)
        Some(std::path::PathBuf::from(format!("icons/{}", icon_name))),
        Some(std::path::PathBuf::from(format!(
            "src-tauri/icons/{}",
            icon_name
        ))),
        // Absolute path for development (tray)
        Some(std::path::PathBuf::from(format!(
            "{}/icons/tray/{}",
            env!("CARGO_MANIFEST_DIR"),
            icon_name
        ))),
        // Absolute path for development (main icons)
        Some(std::path::PathBuf::from(format!(
            "{}/icons/{}",
            env!("CARGO_MANIFEST_DIR"),
            icon_name
        ))),
    ];

    for path in icon_paths.iter().flatten() {
        if path.exists() {
            match Image::from_path(path) {
                Ok(img) => {
                    eprintln!("[Tray] Loaded icon from: {:?}", path);
                    return Some(img.to_owned());
                }
                Err(e) => {
                    eprintln!("[Tray] Failed to load icon from {:?}: {}", path, e);
                }
            }
        }
    }
    None
}

/// Create a fallback tray icon (simple circle).
fn create_fallback_tray_icon(color: (u8, u8, u8)) -> Image<'static> {
    let size = 22u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let center = size as f32 / 2.0 - 0.5;
    let radius = size as f32 / 2.0 - 2.0;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;
            if dist < radius {
                rgba[idx] = color.0; // R
                rgba[idx + 1] = color.1; // G
                rgba[idx + 2] = color.2; // B
                rgba[idx + 3] = 255; // A
            }
        }
    }
    Image::new_owned(rgba, size, size)
}

/// Load the appropriate tray icon for the current desktop environment.
fn load_platform_tray_icon(app: &tauri::App) -> Image<'static> {
    if is_cosmic() {
        // COSMIC: Use full-color icons
        eprintln!("[Tray] COSMIC detected, using full-color icon");
        load_tray_icon(app, icon_names::COLOR_128)
            .or_else(|| load_tray_icon(app, icon_names::COLOR_64))
            .or_else(|| load_tray_icon(app, icon_names::COLOR_32))
            .unwrap_or_else(|| {
                eprintln!("[Tray] Warning: Could not load icon, using fallback");
                create_fallback_tray_icon((59, 130, 246)) // Blue fallback
            })
    } else {
        // GNOME/KDE: Use symbolic (monochrome white) icons
        load_tray_icon(app, icon_names::SYMBOLIC_22)
            .or_else(|| load_tray_icon(app, icon_names::SYMBOLIC_24))
            .or_else(|| load_tray_icon(app, icon_names::SYMBOLIC_32))
            .or_else(|| load_tray_icon(app, icon_names::SYMBOLIC))
            .unwrap_or_else(|| {
                eprintln!("[Tray] Warning: Could not load icon, using fallback");
                create_fallback_tray_icon((255, 255, 255)) // White fallback
            })
    }
}

// =============================================================================
// Tray Setup
// =============================================================================

/// Set up the system tray on Linux.
///
/// On portal-mode desktops (GNOME, KDE, COSMIC), this creates the tray icon
/// and hides the main window. On other Linux desktops (e.g., Hyprland),
/// this is a no-op for now (future: enable tray on all desktops).
pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // For now, only set up tray on portal-mode desktops
    // Future: Enable tray on all Linux desktops
    if !is_portal_mode() {
        return Ok(());
    }

    eprintln!("[Tray] Setting up tray icon for portal-mode desktop...");

    // Load tray icon
    let icon = load_platform_tray_icon(app);

    // Track recording state
    let is_recording = Arc::new(AtomicBool::new(false));

    // Load current always-on-top state from config
    let initial_always_on_top = {
        use crate::config::load_config;
        load_config().always_on_top
    };

    // Create menu items
    let record_item = MenuItem::with_id(
        app,
        menu_ids::RECORD,
        menu_labels::RECORD,
        true,
        None::<&str>,
    )?;
    let stop_item = MenuItem::with_id(
        app,
        menu_ids::STOP,
        menu_labels::STOP,
        false, // Disabled by default (not recording)
        None::<&str>,
    )?;
    let transcription_item = MenuItem::with_id(
        app,
        menu_ids::TRANSCRIPTION,
        menu_labels::TRANSCRIPTION,
        true,
        None::<&str>,
    )?;
    let always_on_top_item = CheckMenuItem::with_id(
        app,
        menu_ids::ALWAYS_ON_TOP,
        menu_labels::ALWAYS_ON_TOP,
        true,
        initial_always_on_top,
        None::<&str>,
    )?;
    let configuration = MenuItem::with_id(
        app,
        menu_ids::CONFIGURATION,
        menu_labels::CONFIGURATION,
        true,
        None::<&str>,
    )?;
    let logs = MenuItem::with_id(app, menu_ids::LOGS, menu_labels::LOGS, true, None::<&str>)?;
    let about = MenuItem::with_id(app, menu_ids::ABOUT, menu_labels::ABOUT, true, None::<&str>)?;
    let exit = MenuItem::with_id(app, menu_ids::EXIT, menu_labels::EXIT, true, None::<&str>)?;

    // Build menu (Always on Top placed after Transcription, before Configuration)
    let menu = Menu::with_items(
        app,
        &[
            &record_item,
            &stop_item,
            &transcription_item,
            &always_on_top_item,
            &configuration,
            &logs,
            &about,
            &exit,
        ],
    )?;

    // Build tray icon
    let tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("OmniRec")
        .on_menu_event(|app, event| match event.id.as_ref() {
            id if id == menu_ids::RECORD => {
                eprintln!("[Tray] Record Screen/Window clicked");
                let _ = app.emit("tray-start-recording", ());
            }
            id if id == menu_ids::STOP => {
                eprintln!("[Tray] Stop Recording clicked");
                let _ = app.emit("tray-stop-recording", ());
            }
            id if id == menu_ids::TRANSCRIPTION => {
                eprintln!("[Tray] Transcription clicked");
                let _ = app.emit("tray-show-transcription", ());
            }
            id if id == menu_ids::ALWAYS_ON_TOP => {
                eprintln!("[Tray] Always on Top clicked");
                super::toggle_always_on_top(app);
            }
            id if id == menu_ids::CONFIGURATION => {
                eprintln!("[Tray] Configuration clicked - opening config window");
                super::open_config_window(app);
            }
            id if id == menu_ids::LOGS => {
                eprintln!("[Tray] Logs clicked - opening log viewer");
                super::open_log_viewer_window(app);
            }
            id if id == menu_ids::ABOUT => {
                eprintln!("[Tray] About clicked - opening about window");
                super::open_about_window(app);
            }
            id if id == menu_ids::EXIT => {
                eprintln!("[Tray] Exit clicked");
                let _ = app.emit("tray-exit", ());
                std::thread::spawn({
                    let app = app.clone();
                    move || {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        app.exit(0);
                    }
                });
            }
            _ => {}
        })
        .build(app)?;

    // Store tray handle and recording state
    app.manage(TrayState {
        tray: std::sync::Mutex::new(tray),
        is_recording,
        always_on_top_item: std::sync::Mutex::new(Some(always_on_top_item)),
    });

    // Hide main window on portal-mode desktops (start with tray only)
    if let Some(window) = app.get_webview_window("main") {
        eprintln!("[Tray] Hiding main window for portal mode");
        let _ = window.hide();
    }

    eprintln!("[Tray] Tray setup complete");
    Ok(())
}

// =============================================================================
// Tray Control
// =============================================================================

/// Set tray icon visibility.
///
/// On Linux portal-mode, the tray is hidden during recording because GNOME's
/// system indicator is used to show recording status and stop recording.
pub fn set_tray_visible(app: &tauri::AppHandle, visible: bool) {
    if let Some(tray_state) = app.try_state::<TrayState>() {
        tray_state
            .is_recording
            .store(!visible, std::sync::atomic::Ordering::SeqCst);
        if let Ok(tray) = tray_state.tray.lock() {
            eprintln!("[Tray] Setting visible: {}", visible);
            match tray.set_visible(visible) {
                Ok(()) => eprintln!("[Tray] set_visible({}) succeeded", visible),
                Err(e) => eprintln!("[Tray] set_visible({}) failed: {:?}", visible, e),
            }
        } else {
            eprintln!("[Tray] Failed to lock tray mutex");
        }
    } else {
        eprintln!("[Tray] No TrayState available");
    }
}
