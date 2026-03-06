//! IPC request types.

use serde::{Deserialize, Serialize};

use crate::security::validation::{
    validate_coordinates, validate_dimensions, validate_monitor_id, validate_source_id,
    validate_window_handle, ValidationError,
};

/// IPC request from client to service.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    // === Enumeration ===
    /// List all capturable windows
    ListWindows,
    /// List all monitors/displays
    ListMonitors,
    /// List all audio sources
    ListAudioSources,

    // === Capture Control ===
    /// Start window capture
    StartWindowCapture { window_handle: isize },
    /// Start full display capture
    StartDisplayCapture {
        monitor_id: String,
        width: u32,
        height: u32,
    },
    /// Start region capture
    StartRegionCapture {
        monitor_id: String,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },
    /// Start portal-based capture (GNOME mode)
    StartPortalCapture,
    /// Stop current recording
    StopRecording,

    // === State Queries ===
    /// Get current recording state
    GetRecordingState,
    /// Get elapsed recording time
    GetElapsedTime,
    /// Subscribe to real-time events
    SubscribeEvents,

    // === Configuration ===
    /// Get current output format
    GetOutputFormat,
    /// Set output format
    SetOutputFormat { format: String },
    /// Get audio configuration
    GetAudioConfig,
    /// Set audio configuration
    SetAudioConfig {
        enabled: bool,
        source_id: Option<String>,
        microphone_id: Option<String>,
        echo_cancellation: bool,
    },

    // === Thumbnails ===
    /// Get window thumbnail
    GetWindowThumbnail { window_handle: isize },
    /// Get display thumbnail
    GetDisplayThumbnail { monitor_id: String },
    /// Get region preview
    GetRegionPreview {
        monitor_id: String,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },

    // === Highlights ===
    /// Show display highlight
    ShowDisplayHighlight {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
    /// Show window highlight
    ShowWindowHighlight { window_handle: isize },
    /// Clear any active highlight
    ClearHighlight,

    // === Picker Compatibility ===
    /// Query current selection (picker protocol)
    QuerySelection,
    /// Validate approval token
    ValidateToken { token: String },
    /// Store approval token
    StoreToken { token: String },

    // === Transcription ===
    /// Get transcription configuration
    GetTranscriptionConfig,
    /// Set transcription configuration
    SetTranscriptionConfig {
        enabled: bool,
        /// Optional path to the whisper model file
        #[serde(skip_serializing_if = "Option::is_none")]
        model_path: Option<String>,
    },
    /// Get transcription status
    GetTranscriptionStatus,
    /// Get pending transcription segments (for live display)
    /// The `since_index` parameter allows incremental fetching:
    /// - Pass 0 to get all segments
    /// - Pass the last segment index + 1 to get only new segments
    GetTranscriptionSegments { since_index: u32 },

    // === Service Control ===
    /// Request service shutdown
    Shutdown,
    /// Ping for health check
    Ping,
}

impl Request {
    /// Validate all parameters in this request.
    ///
    /// Returns Ok(()) if all parameters are valid, or an error describing
    /// the first invalid parameter found.
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Request::StartWindowCapture { window_handle } => {
                validate_window_handle(*window_handle)?;
            }
            Request::StartDisplayCapture {
                monitor_id,
                width,
                height,
            } => {
                validate_monitor_id(monitor_id)?;
                validate_dimensions(*width, *height)?;
            }
            Request::StartRegionCapture {
                monitor_id,
                x,
                y,
                width,
                height,
            } => {
                validate_monitor_id(monitor_id)?;
                validate_coordinates(*x, *y)?;
                validate_dimensions(*width, *height)?;
            }
            Request::GetWindowThumbnail { window_handle } => {
                validate_window_handle(*window_handle)?;
            }
            Request::GetDisplayThumbnail { monitor_id } => {
                validate_monitor_id(monitor_id)?;
            }
            Request::GetRegionPreview {
                monitor_id,
                x,
                y,
                width,
                height,
            } => {
                validate_monitor_id(monitor_id)?;
                validate_coordinates(*x, *y)?;
                validate_dimensions(*width, *height)?;
            }
            Request::ShowDisplayHighlight { x, y, .. } => {
                // Width/height can be negative for highlight bounds
                validate_coordinates(*x, *y)?;
            }
            Request::ShowWindowHighlight { window_handle } => {
                validate_window_handle(*window_handle)?;
            }
            Request::SetAudioConfig {
                source_id,
                microphone_id,
                ..
            } => {
                if let Some(id) = source_id {
                    validate_source_id(id)?;
                }
                if let Some(id) = microphone_id {
                    validate_source_id(id)?;
                }
            }
            // Other requests have no parameters to validate
            _ => {}
        }
        Ok(())
    }
}
