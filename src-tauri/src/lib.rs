//! OmniRec Tauri application.

mod capture;
mod encoder;
mod state;

use capture::{list_monitors, list_windows, show_highlight, CaptureRegion, MonitorInfo, WindowInfo, ThumbnailCapture, ThumbnailResult, get_backend};
use encoder::ensure_ffmpeg_blocking;
use state::{RecordingManager, RecordingResult, RecordingState};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

#[cfg(target_os = "linux")]
use capture::linux;

/// Application state wrapper.
pub struct AppState {
    recording_manager: Arc<Mutex<RecordingManager>>,
    ffmpeg_ready: bool,
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

        // Initialize Linux IPC server in a dedicated thread with its own runtime
        #[cfg(target_os = "linux")]
        {
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
                            // We need to keep this thread alive so the runtime doesn't drop
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
        }

        Self {
            recording_manager: Arc::new(Mutex::new(RecordingManager::new())),
            ffmpeg_ready,
        }
    }
}

/// Get list of capturable windows.
#[tauri::command]
fn get_windows() -> Vec<WindowInfo> {
    list_windows()
}

/// Check screen recording permission status (macOS only).
/// Returns: "granted", "denied", or "unknown" (non-macOS platforms).
/// 
/// This also triggers the permission prompt on first run to ensure
/// the app appears in the Screen Recording permission list.
#[tauri::command]
fn check_screen_recording_permission() -> String {
    #[cfg(target_os = "macos")]
    {
        use capture::macos::MacOSBackend;
        
        // First check if we already have permission
        if MacOSBackend::has_screen_recording_permission() {
            return "granted".to_string();
        }
        
        // If not granted, trigger the prompt to add app to the permission list
        // This causes macOS to show the permission dialog (first time only)
        // and adds the app to System Settings > Screen Recording
        MacOSBackend::trigger_permission_prompt();
        
        // Check again after triggering
        if MacOSBackend::has_screen_recording_permission() {
            "granted".to_string()
        } else {
            "denied".to_string()
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        "unknown".to_string()
    }
}

/// Open System Settings to the Screen Recording privacy pane (macOS only).
#[tauri::command]
fn open_screen_recording_settings() {
    #[cfg(target_os = "macos")]
    {
        // First trigger the permission prompt to ensure the app is in the list
        use capture::macos::MacOSBackend;
        MacOSBackend::trigger_permission_prompt();
        
        // Then open System Settings directly to the Screen Recording pane
        // This URL scheme works on macOS 13+ (Ventura and later)
        // Falls back to Privacy & Security on older versions
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .spawn();
    }
}

/// Get list of available monitors.
#[tauri::command]
fn get_monitors() -> Vec<MonitorInfo> {
    list_monitors()
}

/// Get current recording state.
#[tauri::command]
async fn get_recording_state(state: State<'_, AppState>) -> Result<RecordingState, String> {
    let manager = state.recording_manager.lock().await;
    Ok(manager.get_state().await)
}

/// Start recording the specified window.
#[tauri::command]
async fn start_recording(
    window_handle: isize,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if !state.ffmpeg_ready {
        let err = "FFmpeg is not available. Please restart the application.";
        eprintln!("[start_recording] Error: {}", err);
        return Err(err.to_string());
    }
    let manager = state.recording_manager.lock().await;
    manager.start_recording(window_handle).await.map_err(|e| {
        eprintln!("[start_recording] Error: {}", e);
        e
    })
}

/// Start recording a screen region.
#[tauri::command]
async fn start_region_recording(
    monitor_id: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if !state.ffmpeg_ready {
        let err = "FFmpeg is not available. Please restart the application.";
        eprintln!("[start_region_recording] Error: {}", err);
        return Err(err.to_string());
    }

    let region = CaptureRegion {
        monitor_id,
        x,
        y,
        width,
        height,
    };

    let manager = state.recording_manager.lock().await;
    manager.start_region_recording(region).await.map_err(|e| {
        eprintln!("[start_region_recording] Error: {}", e);
        e
    })
}

/// Start recording an entire display.
#[tauri::command]
async fn start_display_recording(
    monitor_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if !state.ffmpeg_ready {
        let err = "FFmpeg is not available. Please restart the application.";
        eprintln!("[start_display_recording] Error: {}", err);
        return Err(err.to_string());
    }

    // Find the monitor to get its dimensions
    let monitors = list_monitors();
    let monitor = monitors
        .iter()
        .find(|m| m.id == monitor_id)
        .ok_or_else(|| {
            let err = format!("Monitor not found: {}", monitor_id);
            eprintln!("[start_display_recording] Error: {}", err);
            err
        })?;

    let manager = state.recording_manager.lock().await;
    manager
        .start_display_recording(monitor_id, monitor.width, monitor.height)
        .await
        .map_err(|e| {
            eprintln!("[start_display_recording] Error: {}", e);
            e
        })
}

/// Stop the current recording and save the file.
#[tauri::command]
async fn stop_recording(state: State<'_, AppState>) -> Result<RecordingResult, String> {
    let manager = state.recording_manager.lock().await;
    manager.stop_recording().await.map_err(|e| {
        eprintln!("[stop_recording] Error: {}", e);
        e
    })
}

/// Get elapsed recording time in seconds.
#[tauri::command]
async fn get_elapsed_time(state: State<'_, AppState>) -> Result<u64, String> {
    let manager = state.recording_manager.lock().await;
    Ok(manager.get_elapsed_seconds().await)
}

/// Show a highlight border on the specified monitor.
#[tauri::command]
async fn show_display_highlight(
    monitor_id: String,
) -> Result<(), String> {
    // Find the monitor
    let monitors = list_monitors();
    let monitor = monitors
        .iter()
        .find(|m| m.id == monitor_id)
        .ok_or_else(|| format!("Monitor not found: {}", monitor_id))?;

    show_highlight(
        monitor.x,
        monitor.y,
        monitor.width as i32,
        monitor.height as i32,
    );

    Ok(())
}

/// Show a highlight border on the specified window.
#[tauri::command]
async fn show_window_highlight(
    window_handle: isize,
) -> Result<(), String> {
    // Find the window
    let windows = list_windows();
    let window = windows
        .iter()
        .find(|w| w.handle == window_handle)
        .ok_or_else(|| format!("Window not found: {}", window_handle))?;

    // Only show highlight if window has valid dimensions
    if window.width > 0 && window.height > 0 {
        show_highlight(
            window.x,
            window.y,
            window.width as i32,
            window.height as i32,
        );
    }

    Ok(())
}

/// Configure Hyprland window rules for the region selector.
/// This makes the region selector window floating and properly positioned.
#[cfg(target_os = "linux")]
#[tauri::command]
async fn configure_region_selector_window(window_label: String) -> Result<(), String> {
    eprintln!("[configure_region_selector] Configuring Hyprland rules for window: {}", window_label);
    
    // Check if we're on Hyprland
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_err() {
        eprintln!("[configure_region_selector] Not on Hyprland, skipping");
        return Ok(());
    }
    
    // Use hyprctl to add window rules for the region selector
    // We need to match by title since we can't set a custom class in Tauri
    let rules = vec![
        // Make it floating (not tiled)
        "float,title:^(Region Selection)$",
        // No border/gaps for clean overlay
        "noborder,title:^(Region Selection)$",
        "noshadow,title:^(Region Selection)$",
        "noblur,title:^(Region Selection)$",
        // No rounding for sharp selection
        "rounding 0,title:^(Region Selection)$",
        // Treat as opaque to prevent blur effects underneath
        "opaque 1,title:^(Region Selection)$",
        // Disable animations
        "noanim,title:^(Region Selection)$",
    ];
    
    // Execute commands via hyprctl
    for rule in rules {
        let output = std::process::Command::new("hyprctl")
            .args(&["keyword", "windowrulev2", rule])
            .output();
            
        match output {
            Ok(result) => {
                if result.status.success() {
                    eprintln!("[configure_region_selector] Applied: {}", rule);
                } else {
                    let err = String::from_utf8_lossy(&result.stderr);
                    eprintln!("[configure_region_selector] Failed to apply rule: {} - {}", rule, err);
                }
            }
            Err(e) => {
                eprintln!("[configure_region_selector] Failed to execute hyprctl: {}", e);
            }
        }
    }
    
    Ok(())
}

/// Check if running on Hyprland compositor.
#[tauri::command]
fn is_hyprland() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Get the position of the region selector window from Hyprland.
/// This is needed because Tauri's outerPosition() returns (0,0) on Wayland.
#[cfg(target_os = "linux")]
#[tauri::command]
async fn get_region_selector_position() -> Result<(i32, i32, i32, i32), String> {
    use hyprland::data::Clients;
    use hyprland::shared::HyprData;
    
    // Query Hyprland for the region selector window
    let clients = Clients::get().map_err(|e| format!("Failed to get clients: {}", e))?;
    
    for client in clients {
        if client.title == "Region Selection" {
            eprintln!("[get_region_selector_position] Found window at ({}, {}) size {}x{}", 
                client.at.0, client.at.1, client.size.0, client.size.1);
            return Ok((client.at.0 as i32, client.at.1 as i32, client.size.0 as i32, client.size.1 as i32));
        }
    }
    
    Err("Region selector window not found".to_string())
}

/// Stub for non-Linux platforms.
#[cfg(not(target_os = "linux"))]
#[tauri::command]
async fn get_region_selector_position() -> Result<(i32, i32, i32, i32), String> {
    Err("Only available on Linux".to_string())
}

/// Stub for non-Linux platforms.
#[cfg(not(target_os = "linux"))]
#[tauri::command]
async fn configure_region_selector_window(_window_label: String) -> Result<(), String> {
    Ok(())
}

/// Test the portal flow on Linux (for development/debugging).
/// This validates that the picker service is working correctly.
#[cfg(target_os = "linux")]
#[tauri::command]
async fn test_linux_portal(monitor_id: String) -> Result<String, String> {
    eprintln!("[test_linux_portal] Called with monitor_id: {}", monitor_id);
    linux::test_portal_flow(&monitor_id).await
}

/// Stub for non-Linux platforms.
#[cfg(not(target_os = "linux"))]
#[tauri::command]
async fn test_linux_portal(_monitor_id: String) -> Result<String, String> {
    Err("Portal test is only available on Linux".to_string())
}

/// Thumbnail result for JSON serialization.
#[derive(serde::Serialize)]
pub struct ThumbnailResponse {
    /// Base64-encoded JPEG image data
    data: String,
    /// Thumbnail width in pixels
    width: u32,
    /// Thumbnail height in pixels
    height: u32,
}

impl From<ThumbnailResult> for ThumbnailResponse {
    fn from(result: ThumbnailResult) -> Self {
        Self {
            data: result.data,
            width: result.width,
            height: result.height,
        }
    }
}

/// Capture a thumbnail of a window.
///
/// Returns a base64-encoded JPEG image or null if capture fails.
#[tauri::command]
async fn get_window_thumbnail(window_handle: isize) -> Result<Option<ThumbnailResponse>, String> {
    let backend = get_backend();
    match backend.capture_window_thumbnail(window_handle) {
        Ok(result) => Ok(Some(result.into())),
        Err(e) => {
            // Return None for NotImplemented errors (expected on Windows/macOS)
            // Return error for other failures
            if matches!(e, capture::CaptureError::NotImplemented(_)) {
                Ok(None)
            } else {
                Ok(None) // Fail gracefully - show placeholder
            }
        }
    }
}

/// Capture a thumbnail of a display.
///
/// Returns a base64-encoded JPEG image or null if capture fails.
#[tauri::command]
async fn get_display_thumbnail(monitor_id: String) -> Result<Option<ThumbnailResponse>, String> {
    let backend = get_backend();
    match backend.capture_display_thumbnail(&monitor_id) {
        Ok(result) => Ok(Some(result.into())),
        Err(e) => {
            eprintln!("[get_display_thumbnail] Error: {}", e);
            // Return None for NotImplemented errors (expected on Windows/macOS)
            if matches!(e, capture::CaptureError::NotImplemented(_)) {
                Ok(None)
            } else {
                Ok(None) // Fail gracefully - show placeholder
            }
        }
    }
}

/// Capture a preview of a screen region.
///
/// Returns a base64-encoded JPEG image or null if capture fails.
#[tauri::command]
async fn get_region_preview(
    monitor_id: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<Option<ThumbnailResponse>, String> {
    let backend = get_backend();
    match backend.capture_region_preview(&monitor_id, x, y, width, height) {
        Ok(result) => Ok(Some(result.into())),
        Err(e) => {
            eprintln!("[get_region_preview] Error: {}", e);
            // Return None for NotImplemented errors (expected on Windows/macOS)
            if matches!(e, capture::CaptureError::NotImplemented(_)) {
                Ok(None)
            } else {
                Ok(None) // Fail gracefully - show placeholder
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            get_windows,
            get_monitors,
            get_recording_state,
            start_recording,
            start_region_recording,
            start_display_recording,
            stop_recording,
            get_elapsed_time,
            show_display_highlight,
            show_window_highlight,
            configure_region_selector_window,
            get_region_selector_position,
            is_hyprland,
            test_linux_portal,
            check_screen_recording_permission,
            open_screen_recording_settings,
            get_window_thumbnail,
            get_display_thumbnail,
            get_region_preview,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
