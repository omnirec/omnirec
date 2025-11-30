//! Screen Recorder Tauri application.

mod capture;
mod encoder;
#[cfg(windows)]
mod highlight;
mod state;

use capture::{list_monitors, list_windows, CaptureRegion, MonitorInfo, WindowInfo};
use encoder::ensure_ffmpeg_blocking;
use state::{RecordingManager, RecordingResult, RecordingState};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

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

    // Create a region covering the entire monitor (x=0, y=0 relative to monitor)
    let region = CaptureRegion {
        monitor_id,
        x: 0,
        y: 0,
        width: monitor.width,
        height: monitor.height,
    };

    let manager = state.recording_manager.lock().await;
    manager.start_region_recording(region).await
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

    #[cfg(windows)]
    {
        highlight::show_highlight(
            monitor.x,
            monitor.y,
            monitor.width as i32,
            monitor.height as i32,
        );
    }

    #[cfg(not(windows))]
    {
        // TODO: Implement for other platforms
        eprintln!("Display highlight not implemented for this platform");
    }

    Ok(())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
