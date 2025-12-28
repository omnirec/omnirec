//! OmniRec Tauri application.
//!
//! This is the main entry point for the Tauri backend. The code is organized into
//! several modules:
//!
//! - `commands` - Tauri command handlers organized by functionality
//! - `config` - Application configuration persistence
//! - `ipc` - IPC client for communicating with omnirec-service
//! - `platform` - Minimal platform-specific functionality (e.g., macOS permission checks)
//! - `tray` - Cross-platform system tray functionality

mod commands;
mod config;
pub mod ipc;
mod platform;
pub mod tray;

use config::{load_config, AppConfig};
use ipc::ServiceClient;
use std::sync::Arc;
use tokio::sync::Mutex;



// Re-export tray types for use in commands
#[cfg(target_os = "linux")]
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
/// - Service client for communicating with omnirec-service
/// - Application configuration (persisted locally)
/// - Service connection status
pub struct AppState {
    /// IPC client for communicating with the background service.
    pub service_client: Arc<ServiceClient>,
    /// Application configuration (UI preferences, output directory, etc.).
    pub app_config: Arc<Mutex<AppConfig>>,
    /// Whether the service is connected and ready.
    pub service_ready: Arc<std::sync::atomic::AtomicBool>,
}

impl AppState {
    fn new() -> Self {
        // Load configuration
        let app_config = load_config();
        eprintln!("[AppState] Loaded config: {:?}", app_config);

        // Create service client
        let service_client = Arc::new(ServiceClient::new());

        Self {
            service_client,
            app_config: Arc::new(Mutex::new(app_config)),
            service_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Initialize the service connection.
    /// This spawns the service process if needed and waits for it to be ready.
    pub async fn init_service(&self) -> Result<(), String> {
        use std::sync::atomic::Ordering;

        eprintln!("[AppState] Initializing service connection...");

        // Use the ServiceClient's reconnect_or_spawn method which handles everything
        match self.service_client.reconnect_or_spawn().await {
            Ok(()) => {
                self.service_ready.store(true, Ordering::SeqCst);
                eprintln!("[AppState] Service is ready");
                Ok(())
            }
            Err(e) => {
                eprintln!("[AppState] Service failed to start: {}", e);
                Err(format!("Service failed to start: {}", e))
            }
        }
    }

    /// Check if the service is ready.
    pub fn is_service_ready(&self) -> bool {
        self.service_ready.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Ensure the service is connected, reconnecting if necessary.
    pub async fn ensure_service_connected(&self) -> Result<(), String> {
        use std::sync::atomic::Ordering;

        match self.service_client.ensure_connected().await {
            Ok(()) => {
                self.service_ready.store(true, Ordering::SeqCst);
                Ok(())
            }
            Err(e) => {
                self.service_ready.store(false, Ordering::SeqCst);
                Err(e.to_string())
            }
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

            // Set up system tray (all platforms)
            if let Err(e) = tray::setup_tray(app) {
                eprintln!("[Setup] Failed to set up tray: {}", e);
            }

            // Initialize service connection in background
            // The service will be spawned if not already running
            use tauri::Manager;
            let app_state = app.state::<AppState>();
            let service_client = app_state.service_client.clone();
            let service_ready = app_state.service_ready.clone();

            tauri::async_runtime::spawn(async move {
                // Use reconnect_or_spawn which handles all cases:
                // - Service already running: connects directly
                // - Service not running: spawns it and waits
                match service_client.reconnect_or_spawn().await {
                    Ok(()) => {
                        service_ready.store(true, std::sync::atomic::Ordering::SeqCst);
                        eprintln!("[Setup] Service connected and ready");
                    }
                    Err(e) => {
                        eprintln!("[Setup] Failed to connect to service: {}", e);
                    }
                }
            });

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
