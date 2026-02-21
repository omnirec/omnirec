//! Audio configuration commands.
//!
//! Commands for managing audio sources and settings.
//! These commands interact directly with the capture backends and RecordingManager.

use crate::capture;
use crate::config::{save_config as save_config_to_disk, AudioConfig};
use crate::state::get_recording_manager;
use crate::AppState;
use omnirec_common::AudioSource;
use tauri::State;

/// Get list of available audio sources.
#[tauri::command]
pub async fn get_audio_sources(_state: State<'_, AppState>) -> Result<Vec<AudioSource>, String> {
    Ok(capture::list_audio_sources())
}

/// Check if macOS system audio capture is available (requires macOS 13+).
/// Returns true on macOS 13+ with ScreenCaptureKit audio support, false otherwise.
/// On non-macOS platforms, returns false.
#[tauri::command]
pub fn is_system_audio_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::platform::macos::is_system_audio_available()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Get current audio configuration.
/// This returns the local config stored in the Tauri client.
#[tauri::command]
pub async fn get_audio_config(state: State<'_, AppState>) -> Result<AudioConfig, String> {
    let config = state.app_config.lock().await;
    Ok(config.audio.clone())
}

/// Save audio configuration.
/// This updates both the local config and syncs to the RecordingManager.
#[tauri::command]
pub async fn save_audio_config(
    enabled: bool,
    source_id: Option<String>,
    microphone_id: Option<String>,
    echo_cancellation: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Update local config
    {
        let mut config = state.app_config.lock().await;

        config.audio.enabled = enabled;
        config.audio.source_id = source_id.clone();
        config.audio.microphone_id = microphone_id.clone();
        config.audio.echo_cancellation = echo_cancellation;

        // Save to disk
        save_config_to_disk(&config)?;
    }

    // Sync to RecordingManager
    let manager = get_recording_manager();
    let _ = manager
        .set_audio_config(omnirec_common::AudioConfig {
            enabled,
            source_id,
            microphone_id,
            echo_cancellation,
        })
        .await;

    tracing::info!("Saved audio config: enabled={}, aec={}", enabled, echo_cancellation);

    Ok(())
}