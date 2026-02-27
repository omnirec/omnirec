//! Windows system tray implementation.
//!
//! This module provides full tray functionality for Windows.
//! The tray icon appears in the Windows notification area (system tray)
//! and provides recording controls and quick access to configuration.

use super::{icon_names, menu_ids, menu_labels, TrayState};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, WebviewUrl, WebviewWindow,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
};

// =============================================================================
// Window Focus Helper
// =============================================================================

/// Show and bring the main window to the foreground on Windows.
///
/// This uses Win32 APIs to reliably bring the window to the foreground,
/// which is necessary because Tauri's `set_focus()` doesn't always work
/// on Windows when the window is hidden or in the background.
fn show_and_focus_window(window: &WebviewWindow) {
    // First, use Tauri's API to show the window
    let _ = window.show();
    let _ = window.unminimize();

    // Then use Win32 API to bring it to the foreground
    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            // Restore the window if minimized
            let _ = ShowWindow(hwnd, SW_RESTORE);
            // Show the window
            let _ = ShowWindow(hwnd, SW_SHOW);
            // Bring to foreground
            let _ = SetForegroundWindow(hwnd);
        }
    }

    // Also call Tauri's set_focus as a fallback
    let _ = window.set_focus();
}

/// Show the main window on Windows, recreating it if necessary.
///
/// On Windows, transparent/borderless windows can sometimes be destroyed
/// even when hide() is called. This function handles that case by recreating
/// the window if it doesn't exist.
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        // Window exists, just show and focus it
        eprintln!("[Tray] Found existing main window, showing...");
        show_and_focus_window(&window);
    } else {
        // Window was destroyed - recreate it
        eprintln!("[Tray] Main window was destroyed, recreating...");

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
                eprintln!("[Tray] Window recreated successfully");
                // Focus the new window
                show_and_focus_window(&window);
            }
            Err(e) => {
                eprintln!("[Tray] Failed to recreate window: {:?}", e);
            }
        }
    }
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
    let size = 32u32;
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

/// Load the normal (idle) tray icon.
fn load_normal_tray_icon(app: &tauri::App) -> Image<'static> {
    // Windows: Use full-color icons (32x32 preferred for Windows tray)
    load_tray_icon(app, icon_names::COLOR_32)
        .or_else(|| load_tray_icon(app, icon_names::COLOR_64))
        .or_else(|| load_tray_icon(app, icon_names::COLOR_128))
        .unwrap_or_else(|| {
            eprintln!("[Tray] Warning: Could not load normal icon, using fallback");
            create_fallback_tray_icon((59, 130, 246)) // Blue fallback
        })
}

// =============================================================================
// Tray Setup
// =============================================================================

/// Set up the system tray on Windows.
///
/// Creates a tray icon in the Windows notification area with a context menu
/// for recording controls and quick access to configuration.
pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("[Tray] Setting up Windows tray icon...");

    // Load tray icon
    let icon = load_normal_tray_icon(app);

    // Track recording state
    let is_recording = Arc::new(AtomicBool::new(false));

    // Create menu items
    let record_item = MenuItem::with_id(
        app,
        menu_ids::RECORD,
        menu_labels::RECORD,
        true,
        None::<&str>,
    )?;
    let stop_item = MenuItem::with_id(app, menu_ids::STOP, menu_labels::STOP, false, None::<&str>)?;
    let transcription_item = MenuItem::with_id(
        app,
        menu_ids::TRANSCRIPTION,
        menu_labels::TRANSCRIPTION,
        true,
        None::<&str>,
    )?;
    let configuration = MenuItem::with_id(
        app,
        menu_ids::CONFIGURATION,
        menu_labels::CONFIGURATION,
        true,
        None::<&str>,
    )?;
    let about = MenuItem::with_id(app, menu_ids::ABOUT, menu_labels::ABOUT, true, None::<&str>)?;
    let exit = MenuItem::with_id(app, menu_ids::EXIT, menu_labels::EXIT, true, None::<&str>)?;

    // Build menu with all items including Stop Recording and Transcription
    let menu = Menu::with_items(
        app,
        &[
            &record_item,
            &stop_item,
            &transcription_item,
            &configuration,
            &about,
            &exit,
        ],
    )?;

    // Build tray icon
    let tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("OmniRec")
        .on_tray_icon_event(|tray, event| {
            // Handle double-click on tray icon to show/activate main window
            if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                eprintln!("[Tray] Double-click - showing main window");
                show_main_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            id if id == menu_ids::RECORD => {
                // On Windows, show the main window so user can select capture source
                eprintln!("[Tray] Record/Window clicked - showing main window");
                show_main_window(app);
            }
            id if id == menu_ids::STOP => {
                eprintln!("[Tray] Stop Recording clicked");
                let _ = app.emit("tray-stop-recording", ());
            }
            id if id == menu_ids::TRANSCRIPTION => {
                eprintln!("[Tray] Transcription clicked");
                let _ = app.emit("tray-show-transcription", ());
            }
            id if id == menu_ids::CONFIGURATION => {
                eprintln!("[Tray] Configuration clicked - opening config window");
                show_main_window(app);
                let _ = app.emit("tray-show-config", ());
            }
            id if id == menu_ids::ABOUT => {
                eprintln!("[Tray] About clicked - opening about window");
                show_main_window(app);
                let _ = app.emit("tray-show-about", ());
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
    });

    eprintln!("[Tray] Windows tray setup complete");
    Ok(())
}

// =============================================================================
// Tray Control
// =============================================================================

/// Set tray icon visibility on Windows.
///
/// This can be used to hide/show the tray icon as needed.
pub fn set_tray_visible(app: &tauri::AppHandle, visible: bool) {
    if let Some(tray_state) = app.try_state::<TrayState>() {
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

/// Update the tray icon to reflect recording state.
///
/// When recording, the icon changes to a red indicator.
/// When idle, the normal application icon is shown.
pub fn set_recording_state(app: &tauri::AppHandle, recording: bool) {
    use std::sync::atomic::Ordering;

    if let Some(tray_state) = app.try_state::<TrayState>() {
        // Update the recording state flag
        tray_state.is_recording.store(recording, Ordering::SeqCst);

        if let Ok(tray) = tray_state.tray.lock() {
            // Load the appropriate icon
            let icon = if recording {
                load_recording_tray_icon_from_handle(app)
            } else {
                load_normal_tray_icon_from_handle(app)
            };

            // Update the tray icon
            if let Err(e) = tray.set_icon(Some(icon)) {
                eprintln!("[Tray] Failed to update icon: {:?}", e);
            }

            // Update tooltip to reflect state
            let tooltip = if recording {
                "OmniRec - Recording..."
            } else {
                "OmniRec"
            };
            if let Err(e) = tray.set_tooltip(Some(tooltip)) {
                eprintln!("[Tray] Failed to update tooltip: {:?}", e);
            }

            eprintln!("[Tray] Recording state updated: {}", recording);
        }
    }
}

/// Load normal icon using AppHandle (for runtime updates).
fn load_normal_tray_icon_from_handle(app: &tauri::AppHandle) -> Image<'static> {
    load_tray_icon_from_paths(app.path().resource_dir().ok(), icon_names::COLOR_32)
        .or_else(|| load_tray_icon_from_paths(app.path().resource_dir().ok(), icon_names::COLOR_64))
        .or_else(|| {
            load_tray_icon_from_paths(app.path().resource_dir().ok(), icon_names::COLOR_128)
        })
        .unwrap_or_else(|| create_fallback_tray_icon((59, 130, 246)))
}

/// Load recording icon using AppHandle (for runtime updates).
fn load_recording_tray_icon_from_handle(app: &tauri::AppHandle) -> Image<'static> {
    load_tray_icon_from_paths(app.path().resource_dir().ok(), icon_names::RECORDING_32)
        .or_else(|| {
            load_tray_icon_from_paths(app.path().resource_dir().ok(), icon_names::RECORDING_24)
        })
        .or_else(|| {
            load_tray_icon_from_paths(app.path().resource_dir().ok(), icon_names::RECORDING)
        })
        .unwrap_or_else(|| create_fallback_tray_icon((239, 68, 68)))
}
