//! Configuration management for OmniRec.
//!
//! Handles loading and saving user configuration to platform-standard config directories:
//! - Linux: `~/.config/omnirec/config.json`
//! - macOS: `~/Library/Application Support/omnirec/config.json`
//! - Windows: `%APPDATA%\omnirec\config.json`

use directories::ProjectDirs;
use directories::UserDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Theme mode for the application appearance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// Automatically follow system preference
    #[default]
    Auto,
    /// Always use light theme
    Light,
    /// Always use dark theme
    Dark,
}

impl ThemeMode {
    /// Convert from string representation.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }

    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

/// Appearance-related configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    /// Theme mode: auto, light, or dark.
    #[serde(default)]
    pub theme: ThemeMode,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Auto,
        }
    }
}

/// Output-related configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutputConfig {
    /// Custom output directory. If None, uses system default (Videos folder).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
}

/// Audio-related configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Whether audio recording is enabled.
    #[serde(default = "default_audio_enabled")]
    pub enabled: bool,
    /// Selected system audio source ID (output monitor). None means no system audio selected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// Selected microphone source ID. None means no microphone selected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub microphone_id: Option<String>,
    /// Whether echo cancellation is enabled for microphone input.
    #[serde(default = "default_echo_cancellation")]
    pub echo_cancellation: bool,
}

fn default_audio_enabled() -> bool {
    true
}

fn default_echo_cancellation() -> bool {
    true
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            source_id: None, // No source selected by default - user must choose
            microphone_id: None, // No microphone selected by default
            echo_cancellation: true, // AEC enabled by default when mic is used
        }
    }
}

/// Application configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// Output settings group.
    #[serde(default)]
    pub output: OutputConfig,
    /// Audio settings group.
    #[serde(default)]
    pub audio: AudioConfig,
    /// Appearance settings group.
    #[serde(default)]
    pub appearance: AppearanceConfig,
}

impl AppConfig {
    /// Create a new config with default values.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Get the path to the config file.
fn get_config_path() -> Result<PathBuf, String> {
    let proj_dirs = ProjectDirs::from("", "", "omnirec")
        .ok_or("Could not determine config directory")?;
    
    let config_dir = proj_dirs.config_dir();
    Ok(config_dir.join("config.json"))
}

/// Load configuration from disk.
/// Returns default config if file doesn't exist or is invalid.
pub fn load_config() -> AppConfig {
    let config_path = match get_config_path() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("[Config] Failed to get config path: {}", e);
            return AppConfig::default();
        }
    };

    if !config_path.exists() {
        eprintln!("[Config] No config file found, using defaults");
        return AppConfig::default();
    }

    match fs::read_to_string(&config_path) {
        Ok(contents) => {
            match serde_json::from_str::<AppConfig>(&contents) {
                Ok(config) => {
                    eprintln!("[Config] Loaded config from {:?}", config_path);
                    config
                }
                Err(e) => {
                    eprintln!("[Config] Failed to parse config file: {}. Using defaults.", e);
                    AppConfig::default()
                }
            }
        }
        Err(e) => {
            eprintln!("[Config] Failed to read config file: {}. Using defaults.", e);
            AppConfig::default()
        }
    }
}

/// Save configuration to disk.
/// Creates the config directory if it doesn't exist.
pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let config_path = get_config_path()?;
    
    // Ensure config directory exists
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(&config_path, json)
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    eprintln!("[Config] Saved config to {:?}", config_path);
    Ok(())
}

/// Get the default output directory (system Videos folder).
pub fn get_default_output_dir() -> Result<PathBuf, String> {
    let user_dirs = UserDirs::new().ok_or("Could not determine user directories")?;
    
    // Try Videos directory first, fall back to home directory
    let output_dir = user_dirs
        .video_dir()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| {
            let home = user_dirs.home_dir().to_path_buf();
            let videos = home.join("Videos");
            // Try to create Videos directory if it doesn't exist
            if !videos.exists() {
                if fs::create_dir_all(&videos).is_ok() {
                    return videos;
                }
            }
            // Fall back to home directory
            home
        });

    Ok(output_dir)
}

/// Get the configured output directory, falling back to default if not set.
pub fn get_output_dir(config: &AppConfig) -> Result<PathBuf, String> {
    match &config.output.directory {
        Some(dir) if !dir.is_empty() => Ok(PathBuf::from(dir)),
        _ => get_default_output_dir(),
    }
}

/// Validate that a directory exists and is writable.
pub fn validate_directory(path: &str) -> Result<(), String> {
    let path = PathBuf::from(path);
    
    if !path.exists() {
        return Err("Directory does not exist".to_string());
    }
    
    if !path.is_dir() {
        return Err("Path is not a directory".to_string());
    }
    
    // Try to check if writable by creating a temp file
    let test_file = path.join(".omnirec_write_test");
    match fs::write(&test_file, "test") {
        Ok(()) => {
            let _ = fs::remove_file(test_file);
            Ok(())
        }
        Err(_) => Err("Directory is not writable".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert!(config.output.directory.is_none());
        assert!(config.audio.enabled);
        assert!(config.audio.source_id.is_none());
        assert!(config.audio.microphone_id.is_none());
        assert!(config.audio.echo_cancellation);
    }

    #[test]
    fn test_config_serialization() {
        let mut config = AppConfig::default();
        config.output.directory = Some("/custom/path".to_string());
        config.audio.source_id = Some("123".to_string());
        
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.output.directory, Some("/custom/path".to_string()));
        assert_eq!(parsed.audio.source_id, Some("123".to_string()));
    }

    #[test]
    fn test_empty_directory_serialization() {
        // Empty directory should not be serialized
        let config = AppConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        
        // Should not contain "directory" key when None
        assert!(!json.contains("directory"));
    }

    #[test]
    fn test_audio_config_defaults() {
        let config = AudioConfig::default();
        assert!(config.enabled);
        assert!(config.source_id.is_none());
        assert!(config.microphone_id.is_none());
        assert!(config.echo_cancellation);
    }

    #[test]
    fn test_audio_config_serialization() {
        let mut config = AudioConfig::default();
        config.source_id = Some("456".to_string());
        config.microphone_id = Some("789".to_string());
        config.echo_cancellation = false;
        
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AudioConfig = serde_json::from_str(&json).unwrap();
        
        assert!(parsed.enabled);
        assert_eq!(parsed.source_id, Some("456".to_string()));
        assert_eq!(parsed.microphone_id, Some("789".to_string()));
        assert!(!parsed.echo_cancellation);
    }

    #[test]
    fn test_audio_config_backward_compatible() {
        // Test that old config without new fields loads correctly
        let json = r#"{"enabled": true, "source_id": "123"}"#;
        let parsed: AudioConfig = serde_json::from_str(json).unwrap();
        
        assert!(parsed.enabled);
        assert_eq!(parsed.source_id, Some("123".to_string()));
        assert!(parsed.microphone_id.is_none());
        assert!(parsed.echo_cancellation); // default value
    }

    #[test]
    fn test_theme_mode_default() {
        let config = AppearanceConfig::default();
        assert_eq!(config.theme, ThemeMode::Auto);
    }

    #[test]
    fn test_theme_mode_from_str() {
        assert_eq!(ThemeMode::from_str("auto"), Some(ThemeMode::Auto));
        assert_eq!(ThemeMode::from_str("light"), Some(ThemeMode::Light));
        assert_eq!(ThemeMode::from_str("dark"), Some(ThemeMode::Dark));
        assert_eq!(ThemeMode::from_str("AUTO"), Some(ThemeMode::Auto));
        assert_eq!(ThemeMode::from_str("invalid"), None);
    }

    #[test]
    fn test_theme_mode_as_str() {
        assert_eq!(ThemeMode::Auto.as_str(), "auto");
        assert_eq!(ThemeMode::Light.as_str(), "light");
        assert_eq!(ThemeMode::Dark.as_str(), "dark");
    }

    #[test]
    fn test_appearance_config_serialization() {
        let mut config = AppearanceConfig::default();
        config.theme = ThemeMode::Light;
        
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AppearanceConfig = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.theme, ThemeMode::Light);
    }

    #[test]
    fn test_app_config_with_appearance() {
        let mut config = AppConfig::default();
        config.appearance.theme = ThemeMode::Dark;
        
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.appearance.theme, ThemeMode::Dark);
    }

    #[test]
    fn test_app_config_backward_compatible_no_appearance() {
        // Test that old config without appearance field loads correctly
        let json = r#"{"output": {}, "audio": {"enabled": true}}"#;
        let parsed: AppConfig = serde_json::from_str(json).unwrap();
        
        // Should use default appearance
        assert_eq!(parsed.appearance.theme, ThemeMode::Auto);
    }
}
