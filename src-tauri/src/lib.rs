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

use config::{load_config, save_config, AppConfig, LogLevel};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{reload, EnvFilter};

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
// Log line payload (emitted to frontend)
// =============================================================================

/// A single pre-formatted log line sent to the frontend log viewer.
///
/// The line is formatted identically to what `tracing_subscriber::fmt` writes
/// to the log file, so history (read from file) and live events render the same.
///
/// Format: `{timestamp}  {LEVEL} {target}: {message}\n`
/// e.g.  `2026-03-02T00:27:33.464210Z  INFO omnirec_lib: engine started`
#[derive(serde::Serialize, Clone)]
struct LogLinePayload {
    line: String,
}

// =============================================================================
// TauriLogLayer
// =============================================================================

/// A `tracing_subscriber::Layer` that forwards log events to the frontend
/// via a bounded mpsc channel. The channel receiver is drained by a task
/// spawned once `AppHandle` is available (after `Builder::build()`).
struct TauriLogLayer {
    sender: tokio::sync::mpsc::Sender<LogLinePayload>,
}

impl<S> tracing_subscriber::Layer<S> for TauriLogLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        use tracing_subscriber::field::Visit;

        struct MessageVisitor(String);
        impl Visit for MessageVisitor {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.0 = format!("{:?}", value);
                }
            }
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                if field.name() == "message" {
                    self.0 = value.to_string();
                }
            }
        }

        let mut visitor = MessageVisitor(String::new());
        event.record(&mut visitor);

        // Format identically to tracing_subscriber::fmt's default output so that
        // live events and file-read history look the same in the log viewer.
        // File format: "2026-03-02T00:27:33.464210Z  INFO omnirec_lib: message"
        let now = chrono::Utc::now();
        let level = event.metadata().level().to_string().to_uppercase();
        let target = event.metadata().target();
        // Pad level to 5 chars (matching tracing_subscriber's default alignment)
        let line = format!(
            "{}  {:5} {}: {}",
            now.format("%Y-%m-%dT%H:%M:%S%.6fZ"),
            level,
            target,
            visitor.0,
        );
        let payload = LogLinePayload { line };

        // Non-blocking send; drop if channel is full to avoid blocking the caller.
        let _ = self.sender.try_send(payload);
    }
}

// =============================================================================
// Log state (reload handle + channel sender)
// =============================================================================

/// State holding the runtime-reloadable log filter handle.
struct LogState {
    /// Allows reloading the `EnvFilter` at runtime (e.g. from `set_log_level`).
    reload_handle: Arc<reload::Handle<EnvFilter, tracing_subscriber::Registry>>,
}

/// Detect whether we are in development mode (exe path contains debug/release
/// inside a Cargo target directory).
fn is_dev_mode() -> bool {
    std::env::current_exe()
        .ok()
        .map(|p| {
            let s = p.to_string_lossy();
            s.contains("/target/debug/")
                || s.contains("/target/release/")
                || s.contains(r"\target\debug\")
                || s.contains(r"\target\release\")
        })
        .unwrap_or(false)
}

/// Initialize the layered tracing subscriber.
///
/// Returns:
/// - A `LogState` containing the reload handle (stored in Tauri state).
/// - An mpsc receiver for log lines (consumed by a forwarder task after app build).
fn init_logging(
    initial_level: &LogLevel,
) -> (LogState, tokio::sync::mpsc::Receiver<LogLinePayload>) {
    let filter_str = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(initial_level.as_filter_str()));

    let (filter_layer, reload_handle) = reload::Layer::new(filter_str);

    // Bounded channel: 1000 log lines buffered for frontend forwarding.
    let (tx, rx) = tokio::sync::mpsc::channel::<LogLinePayload>(1000);
    let tauri_layer = TauriLogLayer { sender: tx };

    // Both production and development write to the log file.
    // Development additionally writes to stdout with ANSI color.
    if let Err(e) = omnirec_common::logging::ensure_log_dir() {
        // pre-subscriber bootstrap: cannot use tracing yet
        eprintln!(
            "Warning: Failed to create log directory, using temp dir: {}",
            e
        );
    }

    let log_path = omnirec_common::logging::app_log_path();
    let log_dir = log_path.parent().unwrap();

    let file_appender = match tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .max_log_files(5)
        .filename_prefix("omnirec-app")
        .filename_suffix("log")
        .build(log_dir)
    {
        Ok(appender) => appender,
        Err(e) => {
            // pre-subscriber bootstrap: cannot use tracing yet
            eprintln!("Warning: Failed to create log file appender: {}", e);
            let temp_dir = std::env::temp_dir().join("omnirec-logs");
            let _ = std::fs::create_dir_all(&temp_dir);
            tracing_appender::rolling::RollingFileAppender::builder()
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .max_log_files(5)
                .filename_prefix("omnirec-app")
                .filename_suffix("log")
                .build(&temp_dir)
                .expect("Failed to create temp log file appender")
        }
    };

    // Use the rolling appender directly (synchronous) instead of non_blocking.
    // Non-blocking buffers writes in a background channel; forgetting the guard
    // means that buffer is never flushed. Synchronous writes go to the OS
    // immediately, so get_log_history can read them right away.
    let file_fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false);

    if is_dev_mode() {
        // In development: also write to stdout with ANSI color.
        let stdout_fmt_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stdout)
            .with_ansi(true);

        tracing_subscriber::registry()
            .with(filter_layer)
            .with(file_fmt_layer)
            .with(stdout_fmt_layer)
            .with(tauri_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter_layer)
            .with(file_fmt_layer)
            .with(tauri_layer)
            .init();
    }

    let log_state = LogState {
        reload_handle: Arc::new(reload_handle),
    };

    (log_state, rx)
}

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
        info!("[AppState] Loaded config: {:?}", app_config);

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
// Logging Tauri Commands
// =============================================================================

/// Log a startup diagnostic message from the frontend.
#[tauri::command]
fn startup_log(message: String) {
    info!("[Startup/JS] {}", message);
}

/// Log a message from the frontend to the log file at the given level.
#[tauri::command]
fn log_to_file(level: String, message: String) {
    match level.as_str() {
        "error" => error!("{}", message),
        "warn" => warn!("{}", message),
        "info" => info!("{}", message),
        "debug" => debug!("{}", message),
        _ => info!("{}", message),
    }
}

/// Return the raw text of the current session's log file.
///
/// Finds the most recently modified `omnirec-app.*.log` file in the log
/// directory — this is the file the rolling appender is currently writing to.
/// Returns an empty string if no log file exists yet.
#[tauri::command]
fn get_log_history() -> String {
    let log_dir = omnirec_common::logging::app_log_path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("omnirec-logs"));

    // Find the most recently modified omnirec-app.*.log file.
    let most_recent = std::fs::read_dir(&log_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("omnirec-app.") && name.ends_with(".log")
        })
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let modified = meta.modified().ok()?;
            Some((e.path(), modified))
        })
        .max_by_key(|(_, modified)| *modified)
        .map(|(path, _)| path);

    match most_recent {
        Some(path) => std::fs::read_to_string(&path).unwrap_or_default(),
        None => String::new(),
    }
}

/// Get the current log level from config.
#[tauri::command]
fn get_log_level() -> Result<String, String> {
    let config = load_config();
    Ok(config.log_level.as_filter_str().to_string())
}

/// Set the minimum log level at runtime and persist to config.
#[tauri::command]
fn set_log_level(
    level: String,
    state: tauri::State<'_, LogState>,
) -> Result<(), String> {
    let log_level: LogLevel = match level.as_str() {
        "error" => LogLevel::Error,
        "warn" => LogLevel::Warn,
        "info" => LogLevel::Info,
        "debug" => LogLevel::Debug,
        "trace" => LogLevel::Trace,
        other => return Err(format!("Unknown log level: {}", other)),
    };

    // Reload the subscriber filter immediately.
    state
        .reload_handle
        .reload(EnvFilter::new(log_level.as_filter_str()))
        .map_err(|e| format!("Failed to reload log filter: {}", e))?;

    // Persist to config.
    let mut config = load_config();
    config.log_level = log_level;
    save_config(&config).map_err(|e| format!("Failed to save config: {}", e))?;

    Ok(())
}

/// Download all log files as a zip archive via a native save dialog.
#[tauri::command]
async fn download_logs(app_handle: tauri::AppHandle) -> Result<(), String> {
    use std::io::Write;
    use tauri_plugin_dialog::DialogExt;

    let log_dir = omnirec_common::logging::app_log_path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("omnirec-logs"));

    // Collect *.log files.
    let entries: Vec<std::path::PathBuf> = match std::fs::read_dir(&log_dir) {
        Ok(dir) => dir
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("log"))
            .collect(),
        Err(_) => vec![],
    };

    if entries.is_empty() {
        return Err("no_logs".to_string());
    }

    // Build zip in memory.
    let mut zip_buf = std::io::Cursor::new(Vec::<u8>::new());
    {
        let mut zip = zip::ZipWriter::new(&mut zip_buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for path in &entries {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Ok(contents) = std::fs::read(path) {
                    let _ = zip.start_file(name, options);
                    let _ = zip.write_all(&contents);
                }
            }
        }
        zip.finish().map_err(|e| format!("Zip error: {}", e))?;
    }

    let zip_bytes = zip_buf.into_inner();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let default_name = format!("omnirec-logs-{}.zip", today);

    // Show native save dialog and write the bytes.
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<std::path::PathBuf>>();
    app_handle
        .dialog()
        .file()
        .set_file_name(&default_name)
        .save_file(move |path| {
            let _ = tx.send(path.and_then(|p| p.into_path().ok()));
        });

    match rx.await {
        Ok(Some(dest)) => {
            std::fs::write(&dest, &zip_bytes)
                .map_err(|e| format!("Failed to write zip: {}", e))?;
        }
        Ok(None) => {
            // User cancelled — not an error.
        }
        Err(_) => return Err("Dialog channel error".to_string()),
    }

    Ok(())
}

// =============================================================================
// Application Entry Point
// =============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Parse --headless flag from command-line arguments
    let headless = std::env::args().any(|arg| arg == "--headless");

    // Read config before initializing logging so we can use the configured level.
    let initial_config = load_config();

    // Initialize layered logging subscriber with reloadable filter.
    // This must happen before any tracing calls.
    let (log_state, log_rx) = init_logging(&initial_config.log_level);

    if headless {
        info!("[Startup] Running in headless mode (tray only, no main window)");
    }

    #[cfg(target_os = "macos")]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new(headless))
        .manage(log_state);

    #[cfg(not(target_os = "macos"))]
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new(headless))
        .manage(log_state);

    // On macOS, register menu event handler at the Builder level
    // This ensures menu events work even when the window is hidden
    #[cfg(target_os = "macos")]
    {
        builder = builder.on_menu_event(|app, event| {
            tray::macos::handle_menu_event(app, &event);
        });
    }

    let app = builder
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
                    info!("[Setup] Headless: activation policy set to Accessory");
                }
            } else {
                setup_macos_window(app);
            }

            // Set up system tray (all platforms, regardless of headless mode)
            if let Err(e) = tray::setup_tray(app) {
                error!("[Setup] Failed to set up tray: {}", e);
            }

            // Restore always-on-top window property from config
            {
                use tauri::Manager;
                let app_state = app.state::<AppState>();
                // Use a blocking read here since we're still in the synchronous setup closure.
                // We call load_config() directly (same as setup_tray does) to avoid
                // blocking on the async mutex during synchronous setup.
                let always_on_top = {
                    let cfg = config::load_config();
                    cfg.always_on_top
                };
                if always_on_top {
                    if let Some(window) = app.get_webview_window("main") {
                        if let Err(e) = window.set_always_on_top(true) {
                            warn!("[Setup] Failed to set always-on-top: {:?}", e);
                        } else {
                            info!("[Setup] Always-on-top enabled from config");
                        }
                    }
                }
                let _ = app_state; // suppress unused warning
            }

            // ---- Initialize recording subsystem in-process ----
            use tauri::Manager;
            let app_state = app.state::<AppState>();
            let service_ready = app_state.service_ready.clone();
            let app_config = app_state.app_config.clone();

            // Verify FFmpeg is available (bundled sidecar on Windows/macOS,
            // system package on Linux)
            tauri::async_runtime::spawn(async move {
                info!("[Setup] Ensuring FFmpeg is available...");
                match encoder::ensure_ffmpeg_blocking() {
                    Ok(()) => info!("[Setup] FFmpeg check complete"),
                    Err(e) => warn!("[Setup] FFmpeg check failed: {}", e),
                }
            });

            // Initialize platform-specific capture backends (Linux)
            #[cfg(target_os = "linux")]
            {
                capture::linux::init_ipc_server();
                capture::linux::init_screencopy();
                capture::linux::init_audio();
                info!("[Setup] Linux capture backends initialized");
            }

            // Initialize RecordingManager singleton
            let _manager = state::get_recording_manager();
            info!("[Setup] RecordingManager initialized");

            // Sync local config to the RecordingManager
            {
                let config_clone = app_config.clone();
                let service_ready_clone = service_ready.clone();
                tauri::async_runtime::spawn(async move {
                    let config = config_clone.lock().await;

                    // Sync audio config
                    let manager = state::get_recording_manager();
                    info!("[Setup] Syncing audio config: enabled={}, source={:?}, mic={:?}, aec={}",
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
                    info!("[Setup] Syncing transcription config: enabled={}, model={:?}",
                        config.transcription.enabled, model_path
                    );
                    let _ = manager.set_transcription_config(omnirec_common::TranscriptionConfig {
                        enabled: config.transcription.enabled,
                        model_path: Some(model_path.to_string_lossy().to_string()),
                    }).await;

                    service_ready_clone.store(true, Ordering::SeqCst);
                    info!("[Setup] Config sync complete, recording subsystem ready");
                });
            }

            // Start the IPC socket server for CLI communication
            tauri::async_runtime::spawn(async {
                info!("[Setup] Starting IPC socket server...");
                if let Err(e) = ipc::server::run_server().await {
                    error!("[Setup] IPC server error: {}", e);
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
                    info!("[Window] Close requested for main window - hiding (use tray Exit to quit)");
                    api.prevent_close();
                    match window.hide() {
                        Ok(()) => info!("[Window] hide() succeeded"),
                        Err(e) => warn!("[Window] hide() failed: {:?}", e),
                    }

                    // On macOS, set activation policy to Accessory to hide from Dock and Cmd+Tab
                    #[cfg(target_os = "macos")]
                    {
                        use tauri::Manager;
                        let app = window.app_handle();
                        let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                        info!("[Window] Set activation policy to Accessory");
                    }
                } else {
                    info!("[Window] Close requested for window '{}' - allowing close", window.label());
                }
            }

            #[cfg(any(target_os = "windows", target_os = "macos"))]
            if let tauri::WindowEvent::Destroyed = event {
                debug!("[Window] Window '{}' was destroyed", window.label());
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
            // Logging commands
            startup_log,
            log_to_file,
            get_log_history,
            get_log_level,
            set_log_level,
            download_logs,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // Spawn task to drain log lines from TauriLogLayer and forward to all windows.
    {
        let app_handle = app.handle().clone();
        tauri::async_runtime::spawn(async move {
            let mut rx = log_rx;
            while let Some(payload) = rx.recv().await {
                let _ = tauri::Emitter::emit(&app_handle, "log-line", payload);
            }
        });
    }

    app.run(|_app_handle, event| {
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
                info!("[Exit] Application exiting, shutting down subsystems...");
                request_shutdown();

                // Stop active recording if any
                let manager = state::get_recording_manager();
                manager.shutdown();

                // Clean up socket file
                let socket_path = omnirec_common::ipc::get_socket_path();
                if socket_path.exists() {
                    let _ = std::fs::remove_file(&socket_path);
                    info!("[Exit] Removed socket file: {:?}", socket_path);
                }

                info!("[Exit] Shutdown complete");
            }
            _ => {}
        }
    });
}
