//! Audio configuration commands.
//!
//! Commands for managing audio sources and settings.

use crate::capture::AudioSource;
use crate::config::{save_config as save_config_to_disk, AudioConfig};
use crate::AppState;
use tauri::State;

/// Get list of available audio sources.
#[tauri::command]
pub fn get_audio_sources() -> Vec<AudioSource> {
    crate::capture::list_audio_sources()
}

/// Check if macOS system audio capture is available (requires macOS 13+).
/// Returns true on macOS 13+ with ScreenCaptureKit audio support, false otherwise.
/// On non-macOS platforms, returns false.
#[tauri::command]
pub fn is_system_audio_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::capture::macos::audio::is_system_audio_available()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Get current audio configuration.
#[tauri::command]
pub async fn get_audio_config(state: State<'_, AppState>) -> Result<AudioConfig, String> {
    let config = state.app_config.lock().await;
    Ok(config.audio.clone())
}

/// Save audio configuration.
#[tauri::command]
pub async fn save_audio_config(
    enabled: bool,
    source_id: Option<String>,
    microphone_id: Option<String>,
    echo_cancellation: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut config = state.app_config.lock().await;

    config.audio.enabled = enabled;
    config.audio.source_id = source_id;
    config.audio.microphone_id = microphone_id;
    config.audio.echo_cancellation = echo_cancellation;

    // Save to disk
    save_config_to_disk(&config)?;

    eprintln!(
        "[save_audio_config] Saved audio config: enabled={}, source_id={:?}, mic_id={:?}, aec={}",
        config.audio.enabled,
        config.audio.source_id,
        config.audio.microphone_id,
        config.audio.echo_cancellation
    );
    Ok(())
}
