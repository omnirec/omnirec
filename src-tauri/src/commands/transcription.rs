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
        "Saved transcription config: enabled={}, model={:?}",
        enabled,
        model
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
