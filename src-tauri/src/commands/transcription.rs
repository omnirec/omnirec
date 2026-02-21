//! Transcription configuration commands.
//!
//! Commands for managing voice transcription settings.
//! These commands interact directly with the RecordingManager.

use crate::config::{save_config as save_config_to_disk, TranscriptionConfig, WhisperModel};
use crate::state::get_recording_manager;
use crate::AppState;
use futures_util::StreamExt;
use omnirec_common::TranscriptionStatus;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, State};

static DOWNLOAD_CANCEL: AtomicBool = AtomicBool::new(false);
static DOWNLOAD_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Model status information returned by get_model_status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    pub model: String,
    pub display_name: String,
    pub path: String,
    pub exists: bool,
    pub file_size: Option<u64>,
    pub expected_size: u64,
    pub size_display: String,
}

/// Model information for listing available models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub size_bytes: u64,
    pub size_display: String,
    pub description: String,
    pub english_only: bool,
    pub downloaded: bool,
}

/// Download progress event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub model: String,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub percentage: u8,
    pub status: String,
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
/// This updates both the local config and syncs to the RecordingManager.
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

        if let Some(model_str) = &model {
            if let Some(m) = WhisperModel::from_str(model_str) {
                config.transcription.model = m;
            } else {
                return Err(format!("Invalid model: {}", model_str));
            }
        }

        if let Some(show) = show_transcript_window {
            config.transcription.show_transcript_window = show;
        }

        save_config_to_disk(&config)?;

        config.transcription.model.model_path()
    };

    // Sync to RecordingManager
    let manager = get_recording_manager();
    let _ = manager.set_transcription_config(omnirec_common::TranscriptionConfig {
        enabled,
        model_path: Some(model_path.to_string_lossy().to_string()),
    }).await;

    tracing::info!(
        "Saved transcription config: enabled={}, model={:?}, show_transcript_window={:?}",
        enabled,
        model,
        show_transcript_window
    );

    Ok(())
}

/// Get current transcription status from the RecordingManager.
#[tauri::command]
pub async fn get_transcription_status(
    _state: State<'_, AppState>,
) -> Result<TranscriptionStatus, String> {
    let manager = get_recording_manager();
    Ok(manager.get_transcription_status().await)
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
    if DOWNLOAD_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return Err("A download is already in progress".to_string());
    }

    DOWNLOAD_CANCEL.store(false, Ordering::SeqCst);

    let whisper_model =
        WhisperModel::from_str(&model).ok_or_else(|| format!("Invalid model: {}", model))?;

    let url = whisper_model.download_url();
    let path = whisper_model.model_path();
    let model_name = model.clone();

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    let result =
        tokio::spawn(
            async move { download_with_progress(&url, &path, &model_name, &app_handle).await },
        )
        .await
        .map_err(|e| format!("Download task failed: {}", e))?;

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

    let temp_path = path.with_extension("download");
    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_percentage: u8 = 0;

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
        if DOWNLOAD_CANCEL.load(Ordering::SeqCst) {
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

    file.flush()
        .await
        .map_err(|e| format!("Error flushing file: {}", e))?;
    drop(file);

    tokio::fs::rename(&temp_path, path)
        .await
        .map_err(|e| format!("Failed to finalize download: {}", e))?;

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
    _state: State<'_, AppState>,
) -> Result<TranscriptionSegmentsResponse, String> {
    let manager = get_recording_manager();
    let (segments, total_count) = manager.get_transcription_segments(since_index);

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

    if let Some(window) = app.get_webview_window("transcript") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let main_window = app
        .get_webview_window("main")
        .ok_or("Main window not found")?;

    if !main_window.is_visible().unwrap_or(false) {
        tracing::info!("Main window not visible, skipping transcript window");
        return Ok(());
    }

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

    let scale = main_window.scale_factor().map_err(|e| e.to_string())?;

    let main_pos = main_window.outer_position().unwrap_or_default();
    let main_size = main_window.outer_size().unwrap_or_default();

    let main_left = main_pos.x as f64 / scale;
    let main_top = main_pos.y as f64 / scale;
    let main_width = main_size.width as f64 / scale;
    let main_height = main_size.height as f64 / scale;
    let main_right = main_left + main_width;

    let transcript_width = 360.0_f64;
    let transcript_height = main_height;

    let monitor = main_window
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("Could not determine current monitor")?;

    let monitor_pos = monitor.position();
    let monitor_size = monitor.size();

    let screen_left = monitor_pos.x as f64 / scale;
    let screen_right = screen_left + (monitor_size.width as f64 / scale);

    let space_right = screen_right - main_right;
    let space_left = main_left - screen_left;

    let pos_x = if space_right >= transcript_width + gap {
        main_right + gap
    } else if space_left >= transcript_width + gap {
        main_left - transcript_width - gap
    } else {
        main_right + gap
    };

    let pos_y = main_top;

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