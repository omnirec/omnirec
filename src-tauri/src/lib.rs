//! OmniRec Tauri application.
//!
//! This is the main entry point for the Tauri backend. The code is organized into
//! several modules:
//!
//! - `commands` - Tauri command handlers organized by functionality
//! - `config` - Application configuration persistence
//! - `capture` - Platform-specific screen, window, region, and audio capture backends
//! - `encoder` - FFmpeg-based video encoding and transcoding
//! - `transcription` - Whisper.cpp voice transcription
//! - `state` - Recording state management (RecordingManager singleton)
//! - `ipc` - IPC socket server for CLI communication
//! - `platform` - Minimal platform-specific functionality (e.g., macOS permission checks)
//! - `tray` - Cross-platform system tray functionality

mod capture;
mod commands;
mod config;
mod encoder;
pub mod ipc;
mod platform;
pub mod state;
pub mod tray;
mod transcription;

use config::{load_config, AppConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::Mutex;

// =============================================================================
// Shutdown Coordination
// =============================================================================

static SHUTDOWN_FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();

fn get_shutdown_flag() -> Arc<AtomicBool> {
    SHUTDOWN_FLAG
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

/// Request a graceful shutdown of the IPC server and recording subsystem.
pub fn request_shutdown() {
    get_shutdown_flag().store(true, Ordering::SeqCst);
}

/// Check whether a shutdown has been requested.
pub fn is_shutdown_requested() -> bool {
    get_shutdown_flag().load(Ordering::SeqCst)
}

// Re-export tray types for use in commands
pub use tray::TrayState;

// Legacy alias for backwards compatibility
#[cfg(target_os = "linux")]
pub use tray::GnomeTrayState;

// =============================================================================
// Application State
// =============================================================================

/// Application state wrapper.
///
/// This struct holds all the shared state for the application, including:
/// - Application configuration (persisted locally)
/// - Recording subsystem readiness flag
/// - Headless mode flag (started with --headless)
///
/// The RecordingManager is a `'static` singleton accessed via
/// `state::get_recording_manager()` — it is not stored in AppState.
pub struct AppState {
    /// Application configuration (UI preferences, output directory, etc.).
    pub app_config: Arc<Mutex<AppConfig>>,
    /// Whether the recording subsystem is initialized and ready.
    pub service_ready: Arc<AtomicBool>,
    /// Whether the app was launched in headless mode (--headless).
    pub headless: bool,
}

impl AppState {
    fn new(headless: bool) -> Self {
        // Load configuration
        let app_config = load_config();
        eprintln!("[AppState] Loaded config: {:?}", app_config);

        Self {
            app_config: Arc::new(Mutex::new(app_config)),
            service_ready: Arc::new(AtomicBool::new(false)),
            headless,
        }
    }

    /// Check if the recording subsystem is ready.
    pub fn is_service_ready(&self) -> bool {
        self.service_ready.load(Ordering::SeqCst)
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
    // Parse --headless flag from command-line arguments
    let headless = std::env::args().any(|arg| arg == "--headless");
    if headless {
        eprintln!("[Startup] Running in headless mode (tray only, no main window)");
    }

    #[cfg(target_os = "macos")]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new(headless));

    #[cfg(not(target_os = "macos"))]
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new(headless));

    // On macOS, register menu event handler at the Builder level
    // This ensures menu events work even when the window is hidden
    #[cfg(target_os = "macos")]
    {
        builder = builder.on_menu_event(|app, event| {
            tray::macos::handle_menu_event(app, &event);
        });
    }

    builder
        .setup(move |app| {
            // In headless mode, hide/destroy the main window immediately and set
            // macOS activation policy to Accessory (no dock icon).
            if headless {
                use tauri::Manager;
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                    // Destroy the window to free resources — it will be recreated
                    // from the tray "Show" action if the user wants it.
                    let _ = window.destroy();
                }
                #[cfg(target_os = "macos")]
                {
                    let _ = app.handle().set_activation_policy(tauri::ActivationPolicy::Accessory);
                    eprintln!("[Setup] Headless: activation policy set to Accessory");
                }
            } else {
                setup_macos_window(app);
            }

            // Set up system tray (all platforms, regardless of headless mode)
            if let Err(e) = tray::setup_tray(app) {
                eprintln!("[Setup] Failed to set up tray: {}", e);
            }

            // ---- Initialize recording subsystem in-process ----
            use tauri::Manager;
            let app_state = app.state::<AppState>();
            let service_ready = app_state.service_ready.clone();
            let app_config = app_state.app_config.clone();

            // Initialize FFmpeg in a background task (non-fatal, may download on first run)
            tauri::async_runtime::spawn(async move {
                eprintln!("[Setup] Ensuring FFmpeg is available...");
                let _ = encoder::ensure_ffmpeg_blocking();
                eprintln!("[Setup] FFmpeg check complete");
            });

            // Initialize platform-specific capture backends (Linux)
            #[cfg(target_os = "linux")]
            {
                capture::linux::init_ipc_server();
                capture::linux::init_screencopy();
                capture::linux::init_audio();
                eprintln!("[Setup] Linux capture backends initialized");
            }

            // Initialize RecordingManager singleton
            let _manager = state::get_recording_manager();
            eprintln!("[Setup] RecordingManager initialized");

            // Sync local config to the RecordingManager
            {
                let config_clone = app_config.clone();
                let service_ready_clone = service_ready.clone();
                tauri::async_runtime::spawn(async move {
                    let config = config_clone.lock().await;

                    // Sync audio config
                    let manager = state::get_recording_manager();
                    eprintln!("[Setup] Syncing audio config: enabled={}, source={:?}, mic={:?}, aec={}",
                        config.audio.enabled,
                        config.audio.source_id,
                        config.audio.microphone_id,
                        config.audio.echo_cancellation
                    );
                    let _ = manager.set_audio_config(omnirec_common::AudioConfig {
                        enabled: config.audio.enabled,
                        source_id: config.audio.source_id.clone(),
                        microphone_id: config.audio.microphone_id.clone(),
                        echo_cancellation: config.audio.echo_cancellation,
                    }).await;

                    // Sync transcription config
                    let model_path = config.transcription.model.model_path();
                    eprintln!("[Setup] Syncing transcription config: enabled={}, model={:?}",
                        config.transcription.enabled, model_path
                    );
                    let _ = manager.set_transcription_config(omnirec_common::TranscriptionConfig {
                        enabled: config.transcription.enabled,
                        model_path: Some(model_path.to_string_lossy().to_string()),
                    }).await;

                    service_ready_clone.store(true, Ordering::SeqCst);
                    eprintln!("[Setup] Config sync complete, recording subsystem ready");
                });
            }

            // Start the IPC socket server for CLI communication
            tauri::async_runtime::spawn(async {
                eprintln!("[Setup] Starting IPC socket server...");
                if let Err(e) = ipc::server::run_server().await {
                    eprintln!("[Setup] IPC server error: {}", e);
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // On Windows and macOS, hide the main window instead of closing it when the close button
            // is clicked. The app continues running in the system tray/menu bar. Use "Exit" from tray
            // menu to quit. Secondary windows (About, Config, etc.) are allowed to close normally.
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    eprintln!("[Window] Close requested for main window - hiding (use tray Exit to quit)");
                    api.prevent_close();
                    match window.hide() {
                        Ok(()) => eprintln!("[Window] hide() succeeded"),
                        Err(e) => eprintln!("[Window] hide() failed: {:?}", e),
                    }

                    // On macOS, set activation policy to Accessory to hide from Dock and Cmd+Tab
                    #[cfg(target_os = "macos")]
                    {
                        use tauri::Manager;
                        let app = window.app_handle();
                        let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                        eprintln!("[Window] Set activation policy to Accessory");
                    }
                } else {
                    eprintln!("[Window] Close requested for window '{}' - allowing close", window.label());
                }
            }
            
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            if let tauri::WindowEvent::Destroyed = event {
                eprintln!("[Window] Window '{}' was DESTROYED!", window.label());
            }
            
            // Suppress unused variable warning on Linux
            #[cfg(target_os = "linux")]
            let _ = (window, event);
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
            // Service status
            commands::is_service_ready,
            // Transcription commands
            commands::get_transcription_config,
            commands::save_transcription_config,
            commands::get_transcription_status,
            commands::get_transcription_segments,
            commands::open_transcript_window,
            commands::close_transcript_window,
            // Model management commands
            commands::get_model_status,
            commands::list_available_models,
            commands::download_model,
            commands::cancel_download,
            commands::is_download_in_progress,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            match event {
                // On macOS and Windows, prevent the app from exiting when all windows are closed.
                // The app continues running in the system tray/menu bar.
                // Only prevent exit when `code` is None (triggered by window close), not when
                // app.exit(0) is called explicitly (code is Some), so tray Exit works correctly.
                #[cfg(any(target_os = "macos", target_os = "windows"))]
                tauri::RunEvent::ExitRequested { api, code, .. } => {
                    if code.is_none() {
                        api.prevent_exit();
                    }
                }
                // On macOS, when all windows are closed but app is still running,
                // clicking the dock icon should show the main window
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen { .. } => {
                    use tauri::Manager;
                    if let Some(window) = _app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                // On exit, signal shutdown to the IPC server and RecordingManager
                tauri::RunEvent::Exit => {
                    eprintln!("[Exit] Application exiting, shutting down subsystems...");
                    request_shutdown();

                    // Stop active recording if any
                    let manager = state::get_recording_manager();
                    manager.shutdown();

                    // Clean up socket file
                    let socket_path = omnirec_common::ipc::get_socket_path();
                    if socket_path.exists() {
                        let _ = std::fs::remove_file(&socket_path);
                        eprintln!("[Exit] Removed socket file: {:?}", socket_path);
                    }

                    eprintln!("[Exit] Shutdown complete");
                }
                _ => {}
            }
        });
}
