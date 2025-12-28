//! System tray functionality for Linux desktops.
//!
//! This module handles the system tray icon for GNOME, KDE, and COSMIC desktops.
//! These desktops use portal-based recording with a native picker, so the app
//! runs as a tray application without showing the main window.

#[cfg(target_os = "linux")]
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{TrayIcon, TrayIconBuilder},
    Manager,
};

/// State for GNOME system tray (Linux only).
#[cfg(target_os = "linux")]
pub struct GnomeTrayState {
    pub tray: std::sync::Mutex<TrayIcon>,
    pub is_recording: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// Helper to set GNOME tray icon visibility.
#[cfg(target_os = "linux")]
pub fn set_gnome_tray_visible(app: &tauri::AppHandle, visible: bool) {
    if let Some(tray_state) = app.try_state::<GnomeTrayState>() {
        tray_state
            .is_recording
            .store(!visible, std::sync::atomic::Ordering::SeqCst);
        if let Ok(tray) = tray_state.tray.lock() {
            eprintln!("[GNOME Tray] Setting visible: {}", visible);
            match tray.set_visible(visible) {
                Ok(()) => eprintln!("[GNOME Tray] set_visible({}) succeeded", visible),
                Err(e) => eprintln!("[GNOME Tray] set_visible({}) failed: {:?}", visible, e),
            }
        } else {
            eprintln!("[GNOME Tray] Failed to lock tray mutex");
        }
    } else {
        eprintln!("[GNOME Tray] No GnomeTrayState available");
    }
}

#[cfg(not(target_os = "linux"))]
pub fn set_gnome_tray_visible(_app: &tauri::AppHandle, _visible: bool) {
    // No-op on non-Linux
}

/// Check if running on a tray-mode desktop (GNOME, KDE, COSMIC) - internal helper.
/// These desktops use the portal's native picker for source selection.
/// Note: Cinnamon is NOT included because xdg-desktop-portal-xapp does not implement ScreenCast.
#[cfg(target_os = "linux")]
pub fn is_tray_mode_desktop() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| {
            let upper = d.to_uppercase();
            upper.contains("GNOME") || upper.contains("KDE") || upper.contains("COSMIC")
        })
        .unwrap_or(false)
}

/// Check if running on COSMIC desktop (used for icon selection).
#[cfg(target_os = "linux")]
fn is_cosmic() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.to_uppercase().contains("COSMIC"))
        .unwrap_or(false)
}

/// Load a tray icon from multiple possible locations.
/// Returns the icon if found, or None if not found anywhere.
#[cfg(target_os = "linux")]
fn load_tray_icon(app: &tauri::App, icon_name: &str) -> Option<Image<'static>> {
    load_tray_icon_from_paths(app.path().resource_dir().ok(), icon_name)
}

/// Load a tray icon from multiple possible locations given a resource dir.
#[cfg(target_os = "linux")]
fn load_tray_icon_from_paths(
    resource_dir: Option<std::path::PathBuf>,
    icon_name: &str,
) -> Option<Image<'static>> {
    // Try multiple locations for the icon
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
#[cfg(target_os = "linux")]
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

/// Set up system tray for tray-mode desktops (GNOME, KDE, COSMIC).
#[cfg(target_os = "linux")]
pub fn setup_tray_mode(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use tauri::Emitter;

    if !is_tray_mode_desktop() {
        return Ok(());
    }

    eprintln!("[Tray] Setting up tray icon for tray-mode desktop...");

    // Load tray icon
    // COSMIC requires full-color icons; GNOME/KDE work with symbolic (monochrome) icons
    let icon = if is_cosmic() {
        // COSMIC: Try multiple sizes, preferring larger icons
        eprintln!("[Tray] COSMIC detected, using full-color icon");
        load_tray_icon(app, "128x128.png")
            .or_else(|| load_tray_icon(app, "64x64.png"))
            .or_else(|| load_tray_icon(app, "32x32.png"))
            .unwrap_or_else(|| {
                eprintln!("[Tray] Warning: Could not load icon, using fallback");
                create_fallback_tray_icon((59, 130, 246)) // Blue fallback for visibility
            })
    } else {
        // GNOME/KDE: Use symbolic (monochrome white) icons
        load_tray_icon(app, "omnirec-symbolic-22.png")
            .or_else(|| load_tray_icon(app, "omnirec-symbolic-24.png"))
            .or_else(|| load_tray_icon(app, "omnirec-symbolic-32.png"))
            .or_else(|| load_tray_icon(app, "omnirec-symbolic.png"))
            .unwrap_or_else(|| {
                eprintln!("[Tray] Warning: Could not load icon, using fallback");
                create_fallback_tray_icon((255, 255, 255)) // White fallback
            })
    };

    // Track recording state
    let is_recording = Arc::new(AtomicBool::new(false));

    // Create menu items
    let record_item = MenuItem::with_id(app, "record", "Record Screen/Window", true, None::<&str>)?;
    let configuration = MenuItem::with_id(app, "configuration", "Configuration", true, None::<&str>)?;
    let about = MenuItem::with_id(app, "about", "About", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", "Exit", true, None::<&str>)?;

    // Build menu
    let menu = Menu::with_items(app, &[&record_item, &configuration, &about, &exit])?;

    // Build tray icon (hidden during recording, shown when idle)
    let tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("OmniRec")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "record" => {
                eprintln!("[Tray] Record Screen/Window clicked");
                let _ = app.emit("tray-start-recording", ());
            }
            "configuration" => {
                eprintln!("[Tray] Configuration clicked");
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                let _ = app.emit("tray-show-config", ());
            }
            "about" => {
                eprintln!("[Tray] About clicked");
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                let _ = app.emit("tray-show-about", ());
            }
            "exit" => {
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
    app.manage(GnomeTrayState {
        tray: std::sync::Mutex::new(tray),
        is_recording,
    });

    // Hide main window on tray-mode desktops (start with tray only)
    if let Some(window) = app.get_webview_window("main") {
        eprintln!("[Tray] Hiding main window for tray mode");
        let _ = window.hide();
    }

    eprintln!("[Tray] Tray mode setup complete");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn setup_tray_mode(_app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
