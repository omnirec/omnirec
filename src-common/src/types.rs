//! Shared types for cross-platform capture operations.

use serde::{Deserialize, Serialize};

/// Information about a capturable window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    /// Window handle/ID as an integer (platform-specific)
    pub handle: isize,
    /// Window title
    pub title: String,
    /// Process name (executable name)
    pub process_name: String,
    /// Window X position (logical coordinates)
    #[serde(default)]
    pub x: i32,
    /// Window Y position (logical coordinates)
    #[serde(default)]
    pub y: i32,
    /// Window width in pixels
    #[serde(default)]
    pub width: u32,
    /// Window height in pixels
    #[serde(default)]
    pub height: u32,
}

/// Information about a display monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    /// Unique identifier (platform-specific)
    pub id: String,
    /// Display name for UI
    pub name: String,
    /// Virtual screen X position (logical coordinates)
    pub x: i32,
    /// Virtual screen Y position (logical coordinates)
    pub y: i32,
    /// Width in pixels (physical)
    pub width: u32,
    /// Height in pixels (physical)
    pub height: u32,
    /// Whether this is the primary monitor
    pub is_primary: bool,
    /// Scale factor (e.g., 2.0 for Retina displays)
    #[serde(default = "default_scale_factor")]
    pub scale_factor: f64,
}

fn default_scale_factor() -> f64 {
    1.0
}

/// Region specification for capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureRegion {
    /// Monitor device ID
    pub monitor_id: String,
    /// Region X position (relative to monitor, 0-based)
    pub x: i32,
    /// Region Y position (relative to monitor, 0-based)
    pub y: i32,
    /// Region width
    pub width: u32,
    /// Region height
    pub height: u32,
}

/// Information about an audio source (input device or system audio).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSource {
    /// Unique identifier (PipeWire node ID, WASAPI endpoint ID, etc.)
    pub id: String,
    /// Display name for UI
    pub name: String,
    /// Type of audio source
    pub source_type: AudioSourceType,
}

/// Type of audio source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioSourceType {
    /// Microphone or other audio input device
    Input,
    /// System audio / desktop audio (sink monitor)
    Output,
}

/// Output format for recordings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// MP4 container with H.264 codec (default, no transcoding needed)
    #[default]
    Mp4,
    /// WebM container with VP9 codec
    WebM,
    /// MKV container with H.264 codec (remux only)
    Mkv,
    /// QuickTime container with H.264 codec (remux only)
    QuickTime,
    /// Animated GIF
    Gif,
    /// Animated PNG
    AnimatedPng,
    /// Animated WebP
    AnimatedWebp,
}

impl OutputFormat {
    /// Get the file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Mp4 => "mp4",
            OutputFormat::WebM => "webm",
            OutputFormat::Mkv => "mkv",
            OutputFormat::QuickTime => "mov",
            OutputFormat::Gif => "gif",
            OutputFormat::AnimatedPng => "apng",
            OutputFormat::AnimatedWebp => "webp",
        }
    }

    /// Get display name for this format.
    pub fn display_name(&self) -> &'static str {
        match self {
            OutputFormat::Mp4 => "MP4",
            OutputFormat::WebM => "WebM",
            OutputFormat::Mkv => "MKV",
            OutputFormat::QuickTime => "QuickTime (.mov)",
            OutputFormat::Gif => "GIF",
            OutputFormat::AnimatedPng => "Animated PNG",
            OutputFormat::AnimatedWebp => "Animated WebP",
        }
    }

    /// Parse from string (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "mp4" => Some(OutputFormat::Mp4),
            "webm" => Some(OutputFormat::WebM),
            "mkv" => Some(OutputFormat::Mkv),
            "quicktime" | "mov" => Some(OutputFormat::QuickTime),
            "gif" => Some(OutputFormat::Gif),
            "animatedpng" | "apng" => Some(OutputFormat::AnimatedPng),
            "animatedwebp" | "webp" => Some(OutputFormat::AnimatedWebp),
            _ => None,
        }
    }
}

/// Recording state enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordingState {
    /// Not recording, ready to start
    Idle,
    /// Currently recording
    Recording,
    /// Finalizing the recording (encoding remaining frames, writing file)
    Saving,
}

/// Audio configuration for recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Whether audio recording is enabled.
    pub enabled: bool,
    /// Selected system audio source ID (output monitor). None means no system audio selected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// Selected microphone source ID. None means no microphone selected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub microphone_id: Option<String>,
    /// Whether echo cancellation is enabled for microphone input.
    pub echo_cancellation: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            source_id: None,
            microphone_id: None,
            echo_cancellation: true,
        }
    }
}

/// Transcription configuration for recording.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranscriptionConfig {
    /// Whether transcription is enabled.
    #[serde(default)]
    pub enabled: bool,
}

/// Status of the transcription system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionStatus {
    /// Whether the whisper model is loaded.
    pub model_loaded: bool,
    /// Whether transcription is currently active.
    pub active: bool,
    /// Number of segments processed so far.
    pub segments_processed: u32,
    /// Current queue depth (segments waiting to be processed).
    pub queue_depth: u32,
    /// Error message if transcription failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
