//! Transcription configuration commands.
//!
//! Commands for managing voice transcription settings.
//! These commands proxy requests to the omnirec-service via IPC.

use crate::config::{save_config as save_config_to_disk, TranscriptionConfig, WhisperModel};
use crate::AppState;
use futures_util::StreamExt;
use omnirec_common::TranscriptionStatus;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, State};

/// Global download cancellation flag
static DOWNLOAD_CANCEL: AtomicBool = AtomicBool::new(false);

/// Global download in progress flag
static DOWNLOAD_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Model status information returned by get_model_status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    /// Model identifier (e.g., "medium-en")
    pub model: String,
    /// Display name (e.g., "medium.en")
    pub display_name: String,
    /// Expected file path
    pub path: String,
    /// Whether the file exists
    pub exists: bool,
    /// File size on disk (if exists)
    pub file_size: Option<u64>,
    /// Expected download size
    pub expected_size: u64,
    /// Human-readable size
    pub size_display: String,
}

/// Model information for listing available models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier for config (kebab-case, e.g., "medium-en")
    pub id: String,
    /// Display name (e.g., "medium.en")
    pub display_name: String,
    /// Download size in bytes
    pub size_bytes: u64,
    /// Human-readable size (e.g., "1.5 GB")
    pub size_display: String,
    /// Model description
    pub description: String,
    /// Whether this is an English-only model
    pub english_only: bool,
    /// Whether the model is downloaded
    pub downloaded: bool,
}

/// Download progress event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    /// Model being downloaded
    pub model: String,
    /// Bytes downloaded so far
    pub bytes_downloaded: u64,
    /// Total bytes to download
    pub total_bytes: u64,
    /// Progress percentage (0-100)
    pub percentage: u8,
    /// Status: "downloading", "completed", "cancelled", "error"
    pub status: String,
    /// Error message (if status is "error")
    pub error: Option<String>,
}

/// Get current transcription configuration.
/// This returns the local config stored in the Tauri client.
#[tauri::command]
pub async fn get_transcription_config(
    state: State<'_, AppState>,
) -> Result<TranscriptionConfig, String> {
    let config = state.app_config.lock().await;
    Ok(config.transcription.clone())
}

/// Save transcription configuration.
/// This updates both the local config and syncs to the service.
#[tauri::command]
pub async fn save_transcription_config(
    enabled: bool,
    model: Option<String>,
    show_transcript_window: Option<bool>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Update local config and get model path
    let model_path = {
        let mut config = state.app_config.lock().await;
        config.transcription.enabled = enabled;

        // Update model if provided
        if let Some(model_str) = &model {
            if let Some(m) = WhisperModel::from_str(model_str) {
                config.transcription.model = m;
            } else {
                return Err(format!("Invalid model: {}", model_str));
            }
        }

        // Update show_transcript_window if provided
        if let Some(show) = show_transcript_window {
            config.transcription.show_transcript_window = show;
        }

        // Save to disk
        save_config_to_disk(&config)?;

        // Get model path for syncing to service
        config.transcription.model.model_path()
    };

    // Sync to service
    state
        .service_client
        .set_transcription_config(enabled, Some(model_path.to_string_lossy().to_string()))
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!(
        "Saved transcription config: enabled={}, model={:?}, show_transcript_window={:?}",
        enabled,
        model,
        show_transcript_window
    );

    Ok(())
}

/// Get current transcription status from the service.
#[tauri::command]
pub async fn get_transcription_status(
    state: State<'_, AppState>,
) -> Result<TranscriptionStatus, String> {
    state
        .service_client
        .get_transcription_status()
        .await
        .map_err(|e| e.to_string())
}

/// Get status of a specific model (or the currently configured model if not specified)
#[tauri::command]
pub async fn get_model_status(
    model: Option<String>,
    state: State<'_, AppState>,
) -> Result<ModelStatus, String> {
    let whisper_model = if let Some(model_str) = model {
        WhisperModel::from_str(&model_str).ok_or_else(|| format!("Invalid model: {}", model_str))?
    } else {
        let config = state.app_config.lock().await;
        config.transcription.model
    };

    let path = whisper_model.model_path();
    let exists = path.exists();
    let file_size = if exists {
        std::fs::metadata(&path).ok().map(|m| m.len())
    } else {
        None
    };

    Ok(ModelStatus {
        model: format!("{:?}", whisper_model)
            .to_lowercase()
            .replace("en", "-en"),
        display_name: whisper_model.display_name().to_string(),
        path: path.to_string_lossy().to_string(),
        exists,
        file_size,
        expected_size: whisper_model.size_bytes(),
        size_display: whisper_model.size_display().to_string(),
    })
}

/// List all available whisper models with their info
#[tauri::command]
pub async fn list_available_models() -> Result<Vec<ModelInfo>, String> {
    let models: Vec<ModelInfo> = WhisperModel::all()
        .iter()
        .map(|m| ModelInfo {
            id: m.display_name().replace('.', "-"),
            display_name: m.display_name().to_string(),
            size_bytes: m.size_bytes(),
            size_display: m.size_display().to_string(),
            description: m.description().to_string(),
            english_only: m.is_english_only(),
            downloaded: m.is_downloaded(),
        })
        .collect();

    Ok(models)
}

/// Download a whisper model with progress events
#[tauri::command]
pub async fn download_model(model: String, app_handle: tauri::AppHandle) -> Result<(), String> {
    // Check if download is already in progress
    if DOWNLOAD_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return Err("A download is already in progress".to_string());
    }

    // Reset cancel flag
    DOWNLOAD_CANCEL.store(false, Ordering::SeqCst);

    let whisper_model =
        WhisperModel::from_str(&model).ok_or_else(|| format!("Invalid model: {}", model))?;

    let url = whisper_model.download_url();
    let path = whisper_model.model_path();
    let model_name = model.clone();

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    // Spawn download task
    let result =
        tokio::spawn(
            async move { download_with_progress(&url, &path, &model_name, &app_handle).await },
        )
        .await
        .map_err(|e| format!("Download task failed: {}", e))?;

    // Reset in progress flag
    DOWNLOAD_IN_PROGRESS.store(false, Ordering::SeqCst);

    result
}

/// Cancel an in-progress download
#[tauri::command]
pub async fn cancel_download() -> Result<(), String> {
    if !DOWNLOAD_IN_PROGRESS.load(Ordering::SeqCst) {
        return Err("No download in progress".to_string());
    }

    DOWNLOAD_CANCEL.store(true, Ordering::SeqCst);
    tracing::info!("Download cancellation requested");

    Ok(())
}

/// Check if a download is in progress
#[tauri::command]
pub async fn is_download_in_progress() -> Result<bool, String> {
    Ok(DOWNLOAD_IN_PROGRESS.load(Ordering::SeqCst))
}

/// Internal function to download with progress reporting
async fn download_with_progress(
    url: &str,
    path: &std::path::Path,
    model: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    tracing::info!("Starting download from {} to {:?}", url, path);

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to start download: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }

    let total_bytes = response.content_length().unwrap_or(0);

    // Create temp file for download
    let temp_path = path.with_extension("download");
    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_percentage: u8 = 0;

    // Emit initial progress
    let _ = app_handle.emit(
        "model-download-progress",
        DownloadProgress {
            model: model.to_string(),
            bytes_downloaded: 0,
            total_bytes,
            percentage: 0,
            status: "downloading".to_string(),
            error: None,
        },
    );

    use tokio::io::AsyncWriteExt;

    while let Some(chunk_result) = stream.next().await {
        // Check for cancellation
        if DOWNLOAD_CANCEL.load(Ordering::SeqCst) {
            // Clean up temp file
            let _ = tokio::fs::remove_file(&temp_path).await;

            let _ = app_handle.emit(
                "model-download-progress",
                DownloadProgress {
                    model: model.to_string(),
                    bytes_downloaded: downloaded,
                    total_bytes,
                    percentage: last_percentage,
                    status: "cancelled".to_string(),
                    error: None,
                },
            );

            return Err("Download cancelled".to_string());
        }

        let chunk = chunk_result.map_err(|e| format!("Error reading chunk: {}", e))?;

        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Error writing chunk: {}", e))?;

        downloaded += chunk.len() as u64;

        // Calculate percentage and emit progress (at least every 1%)
        let percentage = if total_bytes > 0 {
            ((downloaded as f64 / total_bytes as f64) * 100.0) as u8
        } else {
            0
        };

        if percentage > last_percentage {
            last_percentage = percentage;

            let _ = app_handle.emit(
                "model-download-progress",
                DownloadProgress {
                    model: model.to_string(),
                    bytes_downloaded: downloaded,
                    total_bytes,
                    percentage,
                    status: "downloading".to_string(),
                    error: None,
                },
            );
        }
    }

    // Flush and close file
    file.flush()
        .await
        .map_err(|e| format!("Error flushing file: {}", e))?;
    drop(file);

    // Rename temp file to final path
    tokio::fs::rename(&temp_path, path)
        .await
        .map_err(|e| format!("Failed to finalize download: {}", e))?;

    // Emit completion
    let _ = app_handle.emit(
        "model-download-progress",
        DownloadProgress {
            model: model.to_string(),
            bytes_downloaded: downloaded,
            total_bytes,
            percentage: 100,
            status: "completed".to_string(),
            error: None,
        },
    );

    tracing::info!("Download completed: {:?}", path);

    Ok(())
}

/// Get transcription segments since a given index.
/// Returns segments for live display in the transcript window.
#[tauri::command]
pub async fn get_transcription_segments(
    since_index: u32,
    state: State<'_, AppState>,
) -> Result<TranscriptionSegmentsResponse, String> {
    let (segments, total_count) = state
        .service_client
        .get_transcription_segments(since_index)
        .await
        .map_err(|e| e.to_string())?;

    Ok(TranscriptionSegmentsResponse {
        segments,
        total_count,
    })
}

/// Response for transcription segments request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSegmentsResponse {
    pub segments: Vec<omnirec_common::TranscriptionSegment>,
    pub total_count: u32,
}

/// Open the transcript window
#[tauri::command]
pub async fn open_transcript_window(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    // Check if window already exists
    if let Some(window) = app.get_webview_window("transcript") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Get main window - required for positioning and visibility check
    let main_window = app
        .get_webview_window("main")
        .ok_or("Main window not found")?;

    // Only open transcript window if main window is visible
    if !main_window.is_visible().unwrap_or(false) {
        tracing::info!("Main window not visible, skipping transcript window");
        return Ok(());
    }

    // Determine the URL based on environment
    let url = if cfg!(debug_assertions) {
        WebviewUrl::External(
            "http://localhost:1420/src/transcript-view.html"
                .parse()
                .unwrap(),
        )
    } else {
        WebviewUrl::App("src/transcript-view.html".into())
    };

    let gap = 12.0_f64;

    // Get scale factor first
    let scale = main_window.scale_factor().map_err(|e| e.to_string())?;

    // Get main window position and size (these return physical pixels)
    let main_pos = main_window.outer_position().unwrap_or_default();
    let main_size = main_window.outer_size().unwrap_or_default();

    // Convert to logical pixels
    let main_left = main_pos.x as f64 / scale;
    let main_top = main_pos.y as f64 / scale;
    let main_width = main_size.width as f64 / scale;
    let main_height = main_size.height as f64 / scale;
    let main_right = main_left + main_width;

    // Transcript window dimensions: same height as main, 20% wider than original 300px
    let transcript_width = 360.0_f64;
    let transcript_height = main_height;

    // Get the monitor that the main window is on
    let monitor = main_window
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("Could not determine current monitor")?;

    let monitor_pos = monitor.position();
    let monitor_size = monitor.size();

    // Calculate screen bounds (convert physical to logical)
    let screen_left = monitor_pos.x as f64 / scale;
    let screen_right = screen_left + (monitor_size.width as f64 / scale);

    // Calculate space available on each side
    let space_right = screen_right - main_right;
    let space_left = main_left - screen_left;

    // Determine position: prefer right side, fall back to left if not enough space
    let pos_x = if space_right >= transcript_width + gap {
        // Place on the right
        main_right + gap
    } else if space_left >= transcript_width + gap {
        // Place on the left
        main_left - transcript_width - gap
    } else {
        // Not enough space on either side, place on right anyway (will be partially off-screen)
        main_right + gap
    };

    // Vertical position: align with top of main window
    let pos_y = main_top;

    // Create the transcript window with custom chrome (no OS decorations)
    // Note: transparent windows require the "transcript" window to be listed
    // in capabilities/default.json for drag permissions to work
    let transcript_window = WebviewWindowBuilder::new(&app, "transcript", url)
        .title("Transcript")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(true)
        .accept_first_mouse(true)
        .inner_size(transcript_width, transcript_height)
        .min_inner_size(200.0, 200.0)
        .position(pos_x, pos_y)
        .build()
        .map_err(|e| e.to_string())?;

    // Bring transcript window to front, then restore focus to main window
    // This ensures transcript is visible but main window remains interactive
    let _ = transcript_window.set_focus();
    let _ = main_window.set_focus();

    tracing::info!(
        "Opened transcript window at ({}, {}), space_right={}, space_left={}",
        pos_x,
        pos_y,
        space_right,
        space_left
    );
    Ok(())
}

/// Close the transcript window
#[tauri::command]
pub async fn close_transcript_window(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;

    if let Some(window) = app.get_webview_window("transcript") {
        window.close().map_err(|e| e.to_string())?;
        tracing::info!("Closed transcript window");
    }
    Ok(())
}
