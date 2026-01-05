//! Transcription configuration commands.
//!
//! Commands for managing voice transcription settings.
//! These commands proxy requests to the omnirec-service via IPC.

use crate::config::{save_config as save_config_to_disk, TranscriptionConfig};
use crate::AppState;
use omnirec_common::TranscriptionStatus;
use tauri::State;

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
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Update local config
    {
        let mut config = state.app_config.lock().await;
        config.transcription.enabled = enabled;

        // Save to disk
        save_config_to_disk(&config)?;
    }

    // Sync to service
    state
        .service_client
        .set_transcription_config(enabled)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Saved transcription config: enabled={}", enabled);

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
