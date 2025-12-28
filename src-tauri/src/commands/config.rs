//! Application configuration commands.
//!
//! Commands for managing output directory, theme, and other settings.

use crate::config::{
    get_default_output_dir, save_config as save_config_to_disk, validate_directory, AppConfig,
    ThemeMode,
};
use crate::AppState;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

// =============================================================================
// Response Types
// =============================================================================

/// Configuration response for frontend.
#[derive(serde::Serialize)]
pub struct ConfigResponse {
    pub output: OutputConfigResponse,
    pub audio: AudioConfigResponse,
    pub appearance: AppearanceConfigResponse,
}

#[derive(serde::Serialize)]
pub struct OutputConfigResponse {
    pub directory: Option<String>,
}

#[derive(serde::Serialize)]
pub struct AudioConfigResponse {
    pub enabled: bool,
    pub source_id: Option<String>,
    pub microphone_id: Option<String>,
    pub echo_cancellation: bool,
}

#[derive(serde::Serialize)]
pub struct AppearanceConfigResponse {
    pub theme: String,
}

impl From<&AppConfig> for ConfigResponse {
    fn from(config: &AppConfig) -> Self {
        Self {
            output: OutputConfigResponse {
                directory: config.output.directory.clone(),
            },
            audio: AudioConfigResponse {
                enabled: config.audio.enabled,
                source_id: config.audio.source_id.clone(),
                microphone_id: config.audio.microphone_id.clone(),
                echo_cancellation: config.audio.echo_cancellation,
            },
            appearance: AppearanceConfigResponse {
                theme: config.appearance.theme.as_str().to_string(),
            },
        }
    }
}

// =============================================================================
// Commands
// =============================================================================

/// Get the current application configuration.
#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<ConfigResponse, String> {
    let config = state.app_config.lock().await;
    Ok(ConfigResponse::from(&*config))
}

/// Save the output directory configuration.
#[tauri::command]
pub async fn save_output_directory(
    directory: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Validate directory if provided
    if let Some(ref dir) = directory {
        if !dir.is_empty() {
            validate_directory(dir)?;
        }
    }

    let mut config = state.app_config.lock().await;

    // Update the config
    config.output.directory = directory.filter(|d| !d.is_empty());

    // Save to disk
    save_config_to_disk(&config)?;

    eprintln!(
        "[save_output_directory] Saved output directory: {:?}",
        config.output.directory
    );
    Ok(())
}

/// Get the default output directory (system Videos folder).
#[tauri::command]
pub async fn get_default_output_directory() -> Result<String, String> {
    let dir = get_default_output_dir()?;
    Ok(dir.to_string_lossy().to_string())
}

/// Open a folder picker dialog and return the selected path.
#[tauri::command]
pub async fn pick_output_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();

    app.dialog()
        .file()
        .set_title("Select Output Directory")
        .pick_folder(move |folder_path| {
            let result = folder_path.map(|p| p.to_string());
            let _ = tx.send(result);
        });

    // Wait for dialog result
    let result = rx.recv().map_err(|e| format!("Dialog error: {}", e))?;

    Ok(result)
}

/// Validate a directory path.
#[tauri::command]
pub async fn validate_output_directory(directory: String) -> Result<(), String> {
    validate_directory(&directory)
}

/// Save the theme mode setting.
#[tauri::command]
pub async fn save_theme(theme: String, state: State<'_, AppState>) -> Result<(), String> {
    let theme_mode =
        ThemeMode::from_str(&theme).ok_or_else(|| format!("Invalid theme mode: {}", theme))?;

    let mut config = state.app_config.lock().await;
    config.appearance.theme = theme_mode;

    // Save to disk
    save_config_to_disk(&config)?;

    eprintln!(
        "[save_theme] Saved theme: {}",
        config.appearance.theme.as_str()
    );
    Ok(())
}
