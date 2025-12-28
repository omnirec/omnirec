//! OmniRec Tauri application.
//!
//! This is the main entry point for the Tauri backend. The code is organized into
//! several modules:
//!
//! - `capture` - Screen/window capture backends for each platform
//! - `commands` - Tauri command handlers organized by functionality
//! - `config` - Application configuration persistence
//! - `encoder` - FFmpeg-based video encoding and transcoding
//! - `state` - Recording state management
//! - `tray` - System tray functionality (Linux only)

mod capture;
mod commands;
mod config;
mod encoder;
mod state;
pub mod tray;

use config::{load_config, AppConfig};
use encoder::ensure_ffmpeg_blocking;
use state::RecordingManager;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(target_os = "linux")]
use capture::linux;

// Re-export tray state for use in commands
#[cfg(target_os = "linux")]
pub use tray::GnomeTrayState;

// =============================================================================
// Application State
// =============================================================================

/// Application state wrapper.
///
/// This struct holds all the shared state for the application, including:
/// - Recording manager for controlling active recordings
/// - Application configuration
/// - FFmpeg availability status
pub struct AppState {
    pub recording_manager: Arc<Mutex<RecordingManager>>,
    pub app_config: Arc<Mutex<AppConfig>>,
    pub ffmpeg_ready: bool,
}

impl AppState {
    fn new() -> Self {
        // Initialize FFmpeg at startup (downloads if needed)
        let ffmpeg_ready = match ensure_ffmpeg_blocking() {
            Ok(()) => true,
            Err(e) => {
                eprintln!("Failed to initialize FFmpeg: {}", e);
                false
            }
        };

        // Initialize Linux-specific services
        #[cfg(target_os = "linux")]
        Self::init_linux_services();

        // Load configuration
        let app_config = load_config();
        eprintln!("[AppState] Loaded config: {:?}", app_config);

        Self {
            recording_manager: Arc::new(Mutex::new(RecordingManager::new())),
            app_config: Arc::new(Mutex::new(app_config)),
            ffmpeg_ready,
        }
    }

    /// Initialize Linux-specific services (IPC server, screencopy, audio).
    #[cfg(target_os = "linux")]
    fn init_linux_services() {
        use std::sync::mpsc;

        let (tx, rx) = mpsc::channel();

        // Spawn a thread that will run the IPC server for the lifetime of the app
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                match linux::init_ipc_server().await {
                    Ok(()) => {
                        eprintln!("[AppState] Linux IPC server initialized");
                        tx.send(Ok(())).ok();
                        // Keep the runtime alive - the IPC server runs in a spawned task
                        loop {
                            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
                        }
                    }
                    Err(e) => {
                        eprintln!("[AppState] Failed to init Linux IPC server: {}", e);
                        tx.send(Err(e.clone())).ok();
                    }
                }
            });
        });

        // Wait for IPC server to be ready (with timeout)
        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok(())) => eprintln!("[AppState] IPC server ready"),
            Ok(Err(e)) => eprintln!("[AppState] IPC server failed: {}", e),
            Err(_) => eprintln!("[AppState] Timeout waiting for IPC server"),
        }

        // Pre-initialize screencopy for faster first thumbnail
        linux::init_screencopy();

        // Initialize audio backend
        if let Err(e) = linux::init_audio() {
            eprintln!("[AppState] Failed to init audio backend: {}", e);
        }
    }
}

// =============================================================================
// macOS Window Setup
// =============================================================================

/// Configure macOS window to have rounded corners.
#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn setup_macos_window(app: &tauri::App) {
    use tauri::Manager;

    if let Some(window) = app.get_webview_window("main") {
        use cocoa::appkit::{NSColor, NSWindow};
        use cocoa::base::nil;

        let ns_window = window.ns_window().unwrap() as cocoa::base::id;
        unsafe {
            ns_window.setBackgroundColor_(NSColor::clearColor(nil));
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn setup_macos_window(_app: &tauri::App) {
    // No-op on other platforms
}

// =============================================================================
// Application Entry Point
// =============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .setup(|app| {
            setup_macos_window(app);

            // Set up tray mode if on GNOME, KDE, or COSMIC desktop
            if let Err(e) = tray::setup_tray_mode(app) {
                eprintln!("[Setup] Failed to set up tray mode: {}", e);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Capture commands
            commands::get_windows,
            commands::get_monitors,
            commands::show_display_highlight,
            commands::show_window_highlight,
            commands::get_window_thumbnail,
            commands::get_display_thumbnail,
            commands::get_region_preview,
            commands::check_screen_recording_permission,
            commands::open_screen_recording_settings,
            // Recording commands
            commands::get_recording_state,
            commands::start_recording,
            commands::start_region_recording,
            commands::start_display_recording,
            commands::start_gnome_recording,
            commands::set_tray_recording_state,
            commands::stop_recording,
            commands::get_elapsed_time,
            commands::get_output_format,
            commands::set_output_format,
            // Platform commands
            commands::get_platform,
            commands::is_hyprland,
            commands::is_gnome,
            commands::is_kde,
            commands::is_cosmic,
            commands::is_cinnamon,
            commands::get_desktop_environment,
            commands::configure_region_selector_window,
            commands::get_region_selector_position,
            commands::move_region_selector,
            // Audio commands
            commands::get_audio_sources,
            commands::get_audio_config,
            commands::save_audio_config,
            commands::is_system_audio_available,
            // Configuration commands
            commands::get_config,
            commands::save_output_directory,
            commands::get_default_output_directory,
            commands::pick_output_directory,
            commands::validate_output_directory,
            commands::save_theme,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
