//! Recording control commands.
//!
//! Commands for starting, stopping, and managing screen recordings.
//! These commands proxy requests to the omnirec-service via IPC.

use crate::tray::set_tray_visible;
use crate::AppState;
use omnirec_common::RecordingState;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

/// Result of a completed recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingResult {
    pub success: bool,
    /// Path to the final output file (transcoded if applicable)
    pub file_path: Option<String>,
    /// Path to the original MP4 source file (same as file_path if format is MP4)
    pub source_path: Option<String>,
    pub error: Option<String>,
}

/// Get current recording state.
#[tauri::command]
pub async fn get_recording_state(state: State<'_, AppState>) -> Result<RecordingState, String> {
    state
        .service_client
        .get_recording_state()
        .await
        .map_err(|e| e.to_string())
}

/// Start recording the specified window.
#[tauri::command]
pub async fn start_recording(
    window_handle: isize,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if !state.is_service_ready() {
        return Err("Service is not ready. Please wait for it to start.".to_string());
    }

    state
        .service_client
        .start_window_capture(window_handle)
        .await
        .map_err(|e| {
            tracing::error!("start_recording error: {}", e);
            e.to_string()
        })
}

/// Start recording a screen region.
#[tauri::command]
pub async fn start_region_recording(
    monitor_id: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if !state.is_service_ready() {
        return Err("Service is not ready. Please wait for it to start.".to_string());
    }

    state
        .service_client
        .start_region_capture(monitor_id, x, y, width, height)
        .await
        .map_err(|e| {
            tracing::error!("start_region_recording error: {}", e);
            e.to_string()
        })
}

/// Start recording an entire display.
#[tauri::command]
pub async fn start_display_recording(
    monitor_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if !state.is_service_ready() {
        return Err("Service is not ready. Please wait for it to start.".to_string());
    }

    // Get monitor dimensions from the service
    let monitors = state
        .service_client
        .list_monitors()
        .await
        .map_err(|e| e.to_string())?;

    let monitor = monitors
        .iter()
        .find(|m| m.id == monitor_id)
        .ok_or_else(|| {
            let err = format!("Monitor not found: {}", monitor_id);
            tracing::error!("start_display_recording: {}", err);
            err
        })?;

    state
        .service_client
        .start_display_capture(monitor_id, monitor.width, monitor.height)
        .await
        .map_err(|e| {
            tracing::error!("start_display_recording error: {}", e);
            e.to_string()
        })
}

/// Start recording on GNOME using the standard portal picker.
/// This invokes the xdg-desktop-portal screencast flow with GNOME's native picker.
#[tauri::command]
pub async fn start_gnome_recording(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if !state.is_service_ready() {
        return Err("Service is not ready. Please wait for it to start.".to_string());
    }

    tracing::info!("Starting GNOME portal recording...");

    // Start portal-based recording via IPC
    state
        .service_client
        .start_portal_capture()
        .await
        .map_err(|e| {
            tracing::error!("start_gnome_recording error: {}", e);
            e.to_string()
        })?;

    // Hide tray icon now that recording has started
    set_tray_visible(&app, false);

    // Spawn a background task to monitor the recording state.
    // When PipeWire stream is paused (e.g., user clicks GNOME's indicator),
    // the state changes and we detect this to emit an event to the frontend.
    let app_clone = app.clone();
    let client = state.service_client.clone();

    tokio::spawn(async move {
        tracing::info!("[GNOME] Starting recording state monitor task");

        // Wait a bit for recording to stabilize before monitoring
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Poll the recording state
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            match client.get_recording_state().await {
                Ok(state) => {
                    if state != RecordingState::Recording {
                        tracing::info!("[GNOME] Recording state changed to {:?}", state);
                        // Restore tray icon visibility
                        set_tray_visible(&app_clone, true);
                        let _ = app_clone.emit("recording-stream-stopped", ());
                        tracing::info!("[GNOME] Monitor task exiting");
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("[GNOME] Failed to get recording state: {}", e);
                    // Assume recording stopped on error
                    set_tray_visible(&app_clone, true);
                    let _ = app_clone.emit("recording-stream-stopped", ());
                    break;
                }
            }
        }
    });

    Ok(())
}

/// Update tray icon visibility based on recording state.
/// Update tray icon state based on recording status.
/// - Linux: hide tray icon during recording (system indicator is used to stop).
/// - Windows: change tray icon to recording indicator (red icon).
#[tauri::command]
pub async fn set_tray_recording_state(
    app: tauri::AppHandle,
    recording: bool,
) -> Result<(), String> {
    tracing::debug!(
        "set_tray_recording_state: recording={}",
        recording
    );

    #[cfg(target_os = "linux")]
    {
        use crate::tray::TrayState;
        use std::sync::atomic::Ordering;
        use tauri::Manager;

        // Get the tray state
        if let Some(tray_state) = app.try_state::<TrayState>() {
            tracing::debug!("Got tray state, updating...");
            // Update recording flag
            tray_state.is_recording.store(recording, Ordering::SeqCst);

            if let Ok(tray) = tray_state.tray.lock() {
                tracing::debug!("Setting tray visible: {}", !recording);
                let result = tray.set_visible(!recording);
                tracing::debug!("set_visible result: {:?}", result);
            } else {
                tracing::warn!("Failed to lock tray mutex");
            }
        } else {
            tracing::debug!("No TrayState found (not in portal mode?)");
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Update tray icon to recording state
        crate::tray::set_recording_state(&app, recording);
    }

    #[cfg(target_os = "macos")]
    {
        let _ = (app, recording); // Suppress unused warnings - macOS tray is a stub
    }

    Ok(())
}

/// Stop the current recording and save the file.
/// Transcoding (if needed) is handled by the service.
#[tauri::command]
pub async fn stop_recording(
    _app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<RecordingResult, String> {
    // Stop recording via IPC and get file paths
    // The service handles transcoding internally if a non-MP4 format is configured
    let (file_path, source_path) = state
        .service_client
        .stop_recording()
        .await
        .map_err(|e| {
            tracing::error!("stop_recording error: {}", e);
            e.to_string()
        })?;

    Ok(RecordingResult {
        success: true,
        file_path: Some(file_path),
        source_path: Some(source_path),
        error: None,
    })
}

/// Get elapsed recording time in seconds.
#[tauri::command]
pub async fn get_elapsed_time(state: State<'_, AppState>) -> Result<u64, String> {
    state
        .service_client
        .get_elapsed_time()
        .await
        .map_err(|e| e.to_string())
}
