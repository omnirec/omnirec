//! Configuration management for OmniRec.
//!
//! Handles loading and saving user configuration to platform-standard config directories:
//! - Linux: `~/.config/omnirec/config.json`
//! - macOS: `~/Library/Application Support/omnirec/config.json`
//! - Windows: `%APPDATA%\omnirec\config.json`

use directories::BaseDirs;
use directories::ProjectDirs;
use directories::UserDirs;
use serde::{Deserialize, Serialize};
use std::fmt;
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
            source_id: None,     // No source selected by default - user must choose
            microphone_id: None, // No microphone selected by default
            echo_cancellation: true, // AEC enabled by default when mic is used
        }
    }
}

/// Available Whisper models for transcription.
///
/// Models come in two variants:
/// - English-only (.en suffix): Optimized for English, faster and more accurate for English content
/// - Multilingual: Support multiple languages but slightly less accurate for English
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum WhisperModel {
    /// Tiny English-only model (75 MB)
    TinyEn,
    /// Tiny multilingual model (75 MB)
    Tiny,
    /// Base English-only model (142 MB)
    BaseEn,
    /// Base multilingual model (142 MB)
    Base,
    /// Small English-only model (466 MB)
    SmallEn,
    /// Small multilingual model (466 MB)
    Small,
    /// Medium English-only model (1.5 GB) - default
    #[default]
    MediumEn,
    /// Medium multilingual model (1.5 GB)
    Medium,
    /// Large-v3 multilingual model (2.9 GB) - highest accuracy
    LargeV3,
}

impl WhisperModel {
    /// Get the filename for this model (e.g., "ggml-medium.en.bin")
    pub fn filename(&self) -> &'static str {
        match self {
            Self::TinyEn => "ggml-tiny.en.bin",
            Self::Tiny => "ggml-tiny.bin",
            Self::BaseEn => "ggml-base.en.bin",
            Self::Base => "ggml-base.bin",
            Self::SmallEn => "ggml-small.en.bin",
            Self::Small => "ggml-small.bin",
            Self::MediumEn => "ggml-medium.en.bin",
            Self::Medium => "ggml-medium.bin",
            Self::LargeV3 => "ggml-large-v3.bin",
        }
    }

    /// Get the download URL for this model from Hugging Face
    pub fn download_url(&self) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            self.filename()
        )
    }

    /// Get the approximate download size in bytes
    pub fn size_bytes(&self) -> u64 {
        match self {
            Self::TinyEn | Self::Tiny => 75 * 1024 * 1024, // 75 MB
            Self::BaseEn | Self::Base => 142 * 1024 * 1024, // 142 MB
            Self::SmallEn | Self::Small => 466 * 1024 * 1024, // 466 MB
            Self::MediumEn | Self::Medium => 1536 * 1024 * 1024, // 1.5 GB
            Self::LargeV3 => 2969 * 1024 * 1024,           // 2.9 GB
        }
    }

    /// Get a human-readable size string (e.g., "1.5 GB")
    pub fn size_display(&self) -> &'static str {
        match self {
            Self::TinyEn | Self::Tiny => "75 MB",
            Self::BaseEn | Self::Base => "142 MB",
            Self::SmallEn | Self::Small => "466 MB",
            Self::MediumEn | Self::Medium => "1.5 GB",
            Self::LargeV3 => "2.9 GB",
        }
    }

    /// Get a human-readable display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::TinyEn => "tiny.en",
            Self::Tiny => "tiny",
            Self::BaseEn => "base.en",
            Self::Base => "base",
            Self::SmallEn => "small.en",
            Self::Small => "small",
            Self::MediumEn => "medium.en",
            Self::Medium => "medium",
            Self::LargeV3 => "large-v3",
        }
    }

    /// Get a description of this model
    pub fn description(&self) -> &'static str {
        match self {
            Self::TinyEn => "Fastest, English only",
            Self::Tiny => "Fastest, multilingual",
            Self::BaseEn => "Fast, English only",
            Self::Base => "Fast, multilingual",
            Self::SmallEn => "Balanced, English only",
            Self::Small => "Balanced, multilingual",
            Self::MediumEn => "Accurate, English only (recommended)",
            Self::Medium => "Accurate, multilingual",
            Self::LargeV3 => "Most accurate, multilingual",
        }
    }

    /// Check if this is an English-only model
    pub fn is_english_only(&self) -> bool {
        matches!(
            self,
            Self::TinyEn | Self::BaseEn | Self::SmallEn | Self::MediumEn
        )
    }

    /// Get the path where this model should be stored
    pub fn model_path(&self) -> PathBuf {
        get_whisper_cache_dir().join(self.filename())
    }

    /// Check if the model file exists on disk
    pub fn is_downloaded(&self) -> bool {
        self.model_path().exists()
    }

    /// Get the file size on disk if the model is downloaded
    pub fn file_size(&self) -> Option<u64> {
        std::fs::metadata(self.model_path()).ok().map(|m| m.len())
    }

    /// Get all available models
    pub fn all() -> &'static [WhisperModel] {
        &[
            Self::TinyEn,
            Self::Tiny,
            Self::BaseEn,
            Self::Base,
            Self::SmallEn,
            Self::Small,
            Self::MediumEn,
            Self::Medium,
            Self::LargeV3,
        ]
    }

    /// Parse from string (display name format)
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "tiny.en" | "tiny-en" => Some(Self::TinyEn),
            "tiny" => Some(Self::Tiny),
            "base.en" | "base-en" => Some(Self::BaseEn),
            "base" => Some(Self::Base),
            "small.en" | "small-en" => Some(Self::SmallEn),
            "small" => Some(Self::Small),
            "medium.en" | "medium-en" => Some(Self::MediumEn),
            "medium" => Some(Self::Medium),
            "large-v3" => Some(Self::LargeV3),
            _ => None,
        }
    }
}

impl fmt::Display for WhisperModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Get the whisper model cache directory for the current platform
pub fn get_whisper_cache_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        BaseDirs::new()
            .map(|dirs| dirs.home_dir().join("Library/Caches/omnirec/whisper"))
            .unwrap_or_else(|| PathBuf::from("."))
    } else if cfg!(windows) {
        BaseDirs::new()
            .map(|dirs| dirs.data_local_dir().join("omnirec/whisper"))
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        // Linux and others
        BaseDirs::new()
            .map(|dirs| dirs.cache_dir().join("omnirec/whisper"))
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Transcription-related configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TranscriptionConfig {
    /// Whether transcription is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// The whisper model to use for transcription.
    #[serde(default)]
    pub model: WhisperModel,
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
    /// Transcription settings group.
    #[serde(default)]
    pub transcription: TranscriptionConfig,
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
    let proj_dirs =
        ProjectDirs::from("", "", "omnirec").ok_or("Could not determine config directory")?;

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
        Ok(contents) => match serde_json::from_str::<AppConfig>(&contents) {
            Ok(config) => {
                eprintln!("[Config] Loaded config from {:?}", config_path);
                config
            }
            Err(e) => {
                eprintln!(
                    "[Config] Failed to parse config file: {}. Using defaults.",
                    e
                );
                AppConfig::default()
            }
        },
        Err(e) => {
            eprintln!(
                "[Config] Failed to read config file: {}. Using defaults.",
                e
            );
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

    fs::write(&config_path, json).map_err(|e| format!("Failed to write config file: {}", e))?;

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
            if !videos.exists() && fs::create_dir_all(&videos).is_ok() {
                return videos;
            }
            // Fall back to home directory if Videos exists or creation failed
            if videos.exists() {
                videos
            } else {
                home
            }
        });

    Ok(output_dir)
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
        let config = AudioConfig {
            source_id: Some("456".to_string()),
            microphone_id: Some("789".to_string()),
            echo_cancellation: false,
            ..Default::default()
        };

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
        let config = AppearanceConfig {
            theme: ThemeMode::Light,
        };

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

    #[test]
    fn test_whisper_model_default() {
        let config = TranscriptionConfig::default();
        assert_eq!(config.model, WhisperModel::MediumEn);
        assert!(!config.enabled);
    }

    #[test]
    fn test_whisper_model_serialization() {
        let config = TranscriptionConfig {
            enabled: true,
            model: WhisperModel::SmallEn,
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: TranscriptionConfig = serde_json::from_str(&json).unwrap();

        assert!(parsed.enabled);
        assert_eq!(parsed.model, WhisperModel::SmallEn);
    }

    #[test]
    fn test_whisper_model_backward_compatible() {
        // Test that old config without model field loads correctly
        let json = r#"{"enabled": true}"#;
        let parsed: TranscriptionConfig = serde_json::from_str(json).unwrap();

        assert!(parsed.enabled);
        assert_eq!(parsed.model, WhisperModel::MediumEn); // default
    }

    #[test]
    fn test_whisper_model_metadata() {
        let model = WhisperModel::MediumEn;
        assert_eq!(model.filename(), "ggml-medium.en.bin");
        assert_eq!(model.display_name(), "medium.en");
        assert_eq!(model.size_display(), "1.5 GB");
        assert!(model.is_english_only());

        let model = WhisperModel::LargeV3;
        assert_eq!(model.filename(), "ggml-large-v3.bin");
        assert_eq!(model.display_name(), "large-v3");
        assert!(!model.is_english_only());
    }

    #[test]
    fn test_whisper_model_from_str() {
        assert_eq!(
            WhisperModel::from_str("medium.en"),
            Some(WhisperModel::MediumEn)
        );
        assert_eq!(
            WhisperModel::from_str("medium-en"),
            Some(WhisperModel::MediumEn)
        );
        assert_eq!(
            WhisperModel::from_str("large-v3"),
            Some(WhisperModel::LargeV3)
        );
        assert_eq!(WhisperModel::from_str("tiny"), Some(WhisperModel::Tiny));
        assert_eq!(WhisperModel::from_str("invalid"), None);
    }

    #[test]
    fn test_whisper_model_all() {
        let all = WhisperModel::all();
        assert_eq!(all.len(), 9);
        assert!(all.contains(&WhisperModel::TinyEn));
        assert!(all.contains(&WhisperModel::LargeV3));
    }

    #[test]
    fn test_whisper_model_download_url() {
        let model = WhisperModel::MediumEn;
        assert!(model.download_url().contains("huggingface.co"));
        assert!(model.download_url().contains("ggml-medium.en.bin"));
    }
}
