//! Screen Recorder Tauri application.

mod capture;
mod encoder;
mod state;

use capture::{list_monitors, list_windows, show_highlight, CaptureRegion, MonitorInfo, WindowInfo};
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
            Ok(()) => {
                println!("FFmpeg initialized successfully");
                true
            }
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
        return Err("FFmpeg is not available. Please restart the application.".to_string());
    }
    let manager = state.recording_manager.lock().await;
    manager.start_recording(window_handle).await
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
        return Err("FFmpeg is not available. Please restart the application.".to_string());
    }

    let region = CaptureRegion {
        monitor_id,
        x,
        y,
        width,
        height,
    };

    let manager = state.recording_manager.lock().await;
    manager.start_region_recording(region).await
}

/// Start recording an entire display.
#[tauri::command]
async fn start_display_recording(
    monitor_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if !state.ffmpeg_ready {
        return Err("FFmpeg is not available. Please restart the application.".to_string());
    }

    // Find the monitor to get its dimensions
    let monitors = list_monitors();
    let monitor = monitors
        .iter()
        .find(|m| m.id == monitor_id)
        .ok_or_else(|| format!("Monitor not found: {}", monitor_id))?;

    let manager = state.recording_manager.lock().await;
    manager
        .start_display_recording(monitor_id, monitor.width, monitor.height)
        .await
}

/// Stop the current recording and save the file.
#[tauri::command]
async fn stop_recording(state: State<'_, AppState>) -> Result<RecordingResult, String> {
    let manager = state.recording_manager.lock().await;
    manager.stop_recording().await
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
            test_linux_portal,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
