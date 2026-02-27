//! macOS system tray (menu bar) implementation.
//!
//! This module provides full tray functionality for macOS.
//! The tray icon appears in the macOS menu bar and provides
//! recording controls and quick access to configuration.
//!
//! macOS uses "template" icons that automatically adapt to
//! the menu bar's light/dark appearance.

use super::{icon_names, menu_ids, menu_labels, TrayState};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

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

/// Load the normal (idle) tray icon for macOS.
///
/// Uses template icons that macOS automatically tints based on menu bar appearance.
fn load_normal_tray_icon(app: &tauri::App) -> Image<'static> {
    // macOS: Use template icons (monochrome) for automatic light/dark adaptation
    // Try @2x first for Retina displays, fall back to @1x
    load_tray_icon(app, icon_names::TEMPLATE_2X)
        .or_else(|| load_tray_icon(app, icon_names::TEMPLATE))
        // Fall back to symbolic icons if template not found
        .or_else(|| load_tray_icon(app, icon_names::SYMBOLIC_22))
        .or_else(|| load_tray_icon(app, icon_names::SYMBOLIC))
        .unwrap_or_else(|| {
            eprintln!("[Tray] Warning: Could not load normal icon, using fallback");
            // White fallback for template-style icon
            create_fallback_tray_icon((255, 255, 255))
        })
}

// =============================================================================
// Tray Setup
// =============================================================================

/// Set up the system tray on macOS.
///
/// Creates a menu bar icon with a context menu for recording controls
/// and quick access to configuration.
pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("[Tray] Setting up macOS menu bar icon...");

    // Load tray icon (template icon for automatic light/dark adaptation)
    let icon = load_normal_tray_icon(app);

    // Track recording state
    let is_recording = Arc::new(AtomicBool::new(false));

    // Create menu items
    // Note: macOS "Record Screen/Window" shows the main window (unlike Linux which starts portal)
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
    let configuration = MenuItem::with_id(
        app,
        menu_ids::CONFIGURATION,
        menu_labels::CONFIGURATION,
        true,
        None::<&str>,
    )?;
    let about = MenuItem::with_id(app, menu_ids::ABOUT, menu_labels::ABOUT, true, None::<&str>)?;
    let exit = MenuItem::with_id(app, menu_ids::EXIT, menu_labels::EXIT, true, None::<&str>)?;

    // Build menu with all items
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

    // Build tray icon WITHOUT on_menu_event - we'll handle it at the app level
    let tray = TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(true) // Tell macOS to treat this as a template icon
        .menu(&menu)
        .tooltip("OmniRec")
        .show_menu_on_left_click(true) // Show menu on left click (macOS standard)
        .build(app)?;

    eprintln!("[Tray] Tray icon built successfully");

    // Store tray handle and recording state
    app.manage(TrayState {
        tray: std::sync::Mutex::new(tray),
        is_recording,
    });

    eprintln!("[Tray] macOS menu bar setup complete");
    Ok(())
}

/// Show and activate the main window on macOS.
/// This handles the complexity of bringing a hidden window back to the foreground.
/// If the window was destroyed (which happens on macOS after hide()), it will be recreated.
#[allow(deprecated)] // cocoa crate is deprecated in favor of objc2-app-kit, but still works
fn show_main_window(app: &tauri::AppHandle) {
    use cocoa::appkit::{NSApp, NSApplication};
    use tauri::WebviewUrl;

    // Set activation policy to Regular so app appears in Dock and Cmd+Tab
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    eprintln!("[Tray] Set activation policy to Regular");

    // Activate the app - this is crucial for bringing the app to the foreground
    unsafe {
        let ns_app = NSApp();
        ns_app.activateIgnoringOtherApps_(true);
    }

    if let Some(window) = app.get_webview_window("main") {
        eprintln!("[Tray] Found existing main window, showing...");
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    } else {
        // Window was destroyed after hide() on macOS - recreate it
        eprintln!("[Tray] Main window was destroyed, recreating...");

        match tauri::WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
            .title("OmniRec")
            .inner_size(469.0, 610.0)
            .resizable(false)
            .maximizable(false)
            .decorations(false)
            .transparent(false)
            .shadow(true)
            .accept_first_mouse(true)
            .build()
        {
            Ok(window) => {
                eprintln!("[Tray] Window recreated successfully");
                let _ = window.set_focus();
            }
            Err(e) => {
                eprintln!("[Tray] Failed to recreate window: {:?}", e);
            }
        }
    }
}

/// Handle menu events for macOS tray.
/// This should be called from the app's on_menu_event handler.
pub fn handle_menu_event(app: &tauri::AppHandle, event: &tauri::menu::MenuEvent) {
    let id = event.id.as_ref();
    eprintln!("[Tray] Menu event received: {:?}", id);

    if id == menu_ids::RECORD {
        // macOS: Show the main window so user can select capture source
        eprintln!("[Tray] Record Screen/Window clicked - showing main window");
        show_main_window(app);
    } else if id == menu_ids::STOP {
        eprintln!("[Tray] Stop Recording clicked");
        use tauri::Emitter;
        let _ = app.emit("tray-stop-recording", ());
    } else if id == menu_ids::TRANSCRIPTION {
        eprintln!("[Tray] Transcription clicked");
        use tauri::Emitter;
        let _ = app.emit("tray-show-transcription", ());
    } else if id == menu_ids::CONFIGURATION {
        eprintln!("[Tray] Configuration clicked - opening config window");
        super::open_config_window(app);
    } else if id == menu_ids::ABOUT {
        eprintln!("[Tray] About clicked - opening about window");
        super::open_about_window(app);
    } else if id == menu_ids::EXIT {
        eprintln!("[Tray] Exit clicked, calling app.exit(0)...");
        app.exit(0);
        // Fallback: force exit if app.exit() doesn't work
        eprintln!("[Tray] app.exit() returned, forcing exit with std::process::exit()");
        std::process::exit(0);
    }
}

// =============================================================================
// Tray Control
// =============================================================================

/// Set tray icon visibility on macOS.
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
/// When idle, the normal template icon is shown.
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

            // Recording icons are not templates (they have color)
            // Normal icons are templates (monochrome, auto-tint)
            if let Err(e) = tray.set_icon_as_template(!recording) {
                eprintln!("[Tray] Failed to set icon_as_template: {:?}", e);
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
    load_tray_icon_from_paths(app.path().resource_dir().ok(), icon_names::TEMPLATE_2X)
        .or_else(|| load_tray_icon_from_paths(app.path().resource_dir().ok(), icon_names::TEMPLATE))
        .or_else(|| {
            load_tray_icon_from_paths(app.path().resource_dir().ok(), icon_names::SYMBOLIC_22)
        })
        .or_else(|| load_tray_icon_from_paths(app.path().resource_dir().ok(), icon_names::SYMBOLIC))
        .unwrap_or_else(|| create_fallback_tray_icon((255, 255, 255)))
}

/// Load recording icon using AppHandle (for runtime updates).
fn load_recording_tray_icon_from_handle(app: &tauri::AppHandle) -> Image<'static> {
    load_tray_icon_from_paths(
        app.path().resource_dir().ok(),
        icon_names::RECORDING_TEMPLATE_2X,
    )
    .or_else(|| {
        load_tray_icon_from_paths(
            app.path().resource_dir().ok(),
            icon_names::RECORDING_TEMPLATE,
        )
    })
    .or_else(|| load_tray_icon_from_paths(app.path().resource_dir().ok(), icon_names::RECORDING_22))
    .or_else(|| load_tray_icon_from_paths(app.path().resource_dir().ok(), icon_names::RECORDING))
    .unwrap_or_else(|| create_fallback_tray_icon((239, 68, 68)))
}
