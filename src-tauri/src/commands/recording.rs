//! Recording control commands.
//!
//! Commands for starting, stopping, and managing screen recordings.

use crate::capture::{list_monitors, CaptureRegion};
use crate::state::{OutputFormat, RecordingResult, RecordingState};
use crate::AppState;
use tauri::{Emitter, State};

use crate::tray::set_tray_visible;

/// Get current recording state.
#[tauri::command]
pub async fn get_recording_state(state: State<'_, AppState>) -> Result<RecordingState, String> {
    let manager = state.recording_manager.lock().await;
    Ok(manager.get_state().await)
}

/// Start recording the specified window.
#[tauri::command]
pub async fn start_recording(
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
pub async fn start_region_recording(
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
pub async fn start_display_recording(
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

/// Start recording on GNOME using the standard portal picker.
/// This invokes the xdg-desktop-portal screencast flow with GNOME's native picker.
#[tauri::command]
pub async fn start_gnome_recording(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if !state.ffmpeg_ready {
        let err = "FFmpeg is not available. Please restart the application.";
        eprintln!("[start_gnome_recording] Error: {}", err);
        return Err(err.to_string());
    }

    eprintln!("[start_gnome_recording] Starting GNOME portal recording...");

    // On GNOME, we use portal-based recording.
    // The portal will show the native picker for source selection.
    // We need to get the stop flag while holding the lock, then release it.
    let stop_flag = {
        let manager = state.recording_manager.lock().await;
        manager.start_gnome_portal_recording().await.map_err(|e| {
            eprintln!("[start_gnome_recording] Error: {}", e);
            e
        })?;
        // Get stop flag before releasing the lock
        manager.get_stop_flag().await
    }; // Lock released here

    // Hide tray icon now that recording has started
    set_tray_visible(&app, false);

    // Spawn a background task to monitor the stop flag.
    // When PipeWire stream is paused (e.g., user clicks GNOME's indicator),
    // the stop flag is set. We detect this and emit an event to the frontend.
    if let Some(stop_flag) = stop_flag {
        let app_clone = app.clone();
        tokio::spawn(async move {
            eprintln!("[GNOME] Starting stop flag monitor task");

            // Wait a bit for recording to stabilize before monitoring
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            // Poll the stop flag
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    eprintln!("[GNOME] Stop flag detected in monitor task");
                    // Restore tray icon visibility
                    set_tray_visible(&app_clone, true);
                    let _ = app_clone.emit("recording-stream-stopped", ());
                    eprintln!("[GNOME] Monitor task exiting");
                    break;
                }
            }
        });
    } else {
        eprintln!("[start_gnome_recording] Warning: No stop flag available for monitoring");
    }

    Ok(())
}

/// Update tray icon visibility based on recording state.
/// When recording: hide tray icon (system indicator is used to stop).
/// When idle: show tray icon with menu.
#[tauri::command]
pub async fn set_tray_recording_state(
    app: tauri::AppHandle,
    recording: bool,
) -> Result<(), String> {
    eprintln!(
        "[set_tray_recording_state] Setting recording state: {}, visible: {}",
        recording, !recording
    );

    #[cfg(target_os = "linux")]
    {
        use crate::tray::TrayState;
        use std::sync::atomic::Ordering;
        use tauri::Manager;

        // Get the tray state
        if let Some(tray_state) = app.try_state::<TrayState>() {
            eprintln!("[set_tray_recording_state] Got tray state, updating...");
            // Update recording flag
            tray_state.is_recording.store(recording, Ordering::SeqCst);

            if let Ok(tray) = tray_state.tray.lock() {
                eprintln!(
                    "[set_tray_recording_state] Setting tray visible: {}",
                    !recording
                );
                let result = tray.set_visible(!recording);
                eprintln!("[set_tray_recording_state] set_visible result: {:?}", result);
            } else {
                eprintln!("[set_tray_recording_state] Failed to lock tray mutex");
            }
        } else {
            eprintln!("[set_tray_recording_state] No TrayState found (not in portal mode?)");
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (app, recording); // Suppress unused warnings
    }

    Ok(())
}

/// Stop the current recording and save the file.
/// If the output format is not MP4, transcodes to the target format.
#[tauri::command]
pub async fn stop_recording(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<RecordingResult, String> {
    // Stop recording and get source MP4 path and target format
    let (source_path, format) = {
        let manager = state.recording_manager.lock().await;
        manager.stop_recording().await.map_err(|e| {
            eprintln!("[stop_recording] Error: {}", e);
            e
        })?
    };

    let source_path_str = source_path.to_string_lossy().to_string();

    // If format is MP4, no transcoding needed
    if format == OutputFormat::Mp4 {
        // Reset state to idle
        let manager = state.recording_manager.lock().await;
        manager.set_idle().await;

        return Ok(RecordingResult {
            success: true,
            file_path: Some(source_path_str.clone()),
            source_path: Some(source_path_str),
            error: None,
        });
    }

    // Emit transcoding-started event
    let format_name = format.display_name().to_string();
    let _ = app.emit("transcoding-started", &format_name);
    eprintln!("[stop_recording] Starting transcoding to {}", format_name);

    // Transcode to target format
    let transcode_result = tokio::task::spawn_blocking({
        let source = source_path.clone();
        move || crate::encoder::transcode_video(&source, format)
    })
    .await;

    // Reset state to idle
    {
        let manager = state.recording_manager.lock().await;
        manager.set_idle().await;
    }

    match transcode_result {
        Ok(Ok(output_path)) => {
            let output_path_str = output_path.to_string_lossy().to_string();

            // Emit transcoding-complete event with success
            let _ = app.emit(
                "transcoding-complete",
                serde_json::json!({
                    "success": true,
                    "output_path": &output_path_str,
                    "source_path": &source_path_str,
                }),
            );

            Ok(RecordingResult {
                success: true,
                file_path: Some(output_path_str),
                source_path: Some(source_path_str),
                error: None,
            })
        }
        Ok(Err(e)) => {
            eprintln!("[stop_recording] Transcoding failed: {}", e);

            // Emit transcoding-complete event with failure
            let _ = app.emit(
                "transcoding-complete",
                serde_json::json!({
                    "success": false,
                    "error": &e,
                    "source_path": &source_path_str,
                }),
            );

            // Return success with the original MP4 path, but note the transcoding error
            Ok(RecordingResult {
                success: true, // MP4 was saved successfully
                file_path: Some(source_path_str.clone()),
                source_path: Some(source_path_str),
                error: Some(format!("Transcoding failed: {}. Original MP4 saved.", e)),
            })
        }
        Err(e) => {
            eprintln!("[stop_recording] Transcoding task error: {}", e);

            // Emit transcoding-complete event with failure
            let _ = app.emit(
                "transcoding-complete",
                serde_json::json!({
                    "success": false,
                    "error": e.to_string(),
                    "source_path": &source_path_str,
                }),
            );

            Ok(RecordingResult {
                success: true, // MP4 was saved successfully
                file_path: Some(source_path_str.clone()),
                source_path: Some(source_path_str),
                error: Some(format!(
                    "Transcoding task failed: {}. Original MP4 saved.",
                    e
                )),
            })
        }
    }
}

/// Get elapsed recording time in seconds.
#[tauri::command]
pub async fn get_elapsed_time(state: State<'_, AppState>) -> Result<u64, String> {
    let manager = state.recording_manager.lock().await;
    Ok(manager.get_elapsed_seconds().await)
}

/// Get the current output format.
#[tauri::command]
pub async fn get_output_format(state: State<'_, AppState>) -> Result<String, String> {
    let manager = state.recording_manager.lock().await;
    let format = manager.get_output_format().await;
    Ok(format.extension().to_string())
}

/// Set the output format for future recordings.
#[tauri::command]
pub async fn set_output_format(format: String, state: State<'_, AppState>) -> Result<(), String> {
    let output_format =
        OutputFormat::from_str(&format).ok_or_else(|| format!("Invalid output format: {}", format))?;

    let manager = state.recording_manager.lock().await;
    manager.set_output_format(output_format).await
}
