//! IPC response types.

use serde::{Deserialize, Serialize};

use crate::types::{
    AudioConfig, AudioSource, MonitorInfo, RecordingState, TranscriptionConfig,
    TranscriptionSegment, TranscriptionStatus, WindowInfo,
};

/// Geometry for region selection (picker compatibility).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// IPC response from service to client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    // === Success Responses ===
    /// List of windows
    Windows { windows: Vec<WindowInfo> },
    /// List of monitors
    Monitors { monitors: Vec<MonitorInfo> },
    /// List of audio sources
    AudioSources { sources: Vec<AudioSource> },

    /// Current recording state
    RecordingState { state: RecordingState },
    /// Recording started successfully
    RecordingStarted,
    /// Recording stopped, files saved
    RecordingStopped {
        file_path: String,
        source_path: String,
    },
    /// Elapsed time in seconds
    ElapsedTime { seconds: u64 },

    /// Current output format
    OutputFormat { format: String },
    /// Current audio configuration
    AudioConfig(AudioConfig),
    /// Current transcription configuration
    TranscriptionConfig(TranscriptionConfig),
    /// Current transcription status
    TranscriptionStatus(TranscriptionStatus),
    /// Transcription segments (for live display)
    TranscriptionSegments {
        /// The segments (may be empty if no new segments)
        segments: Vec<TranscriptionSegment>,
        /// The total number of segments produced so far
        total_count: u32,
    },

    /// Thumbnail data (base64-encoded image)
    Thumbnail {
        data: String,
        width: u32,
        height: u32,
    },

    /// Subscribed to events
    Subscribed,
    /// Generic success
    Ok,
    /// Pong response to ping
    Pong,

    // === Selection Responses (Picker Compatibility) ===
    /// Current selection info
    Selection {
        source_type: String,
        source_id: String,
        has_approval_token: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        geometry: Option<SelectionGeometry>,
    },
    /// No selection currently set
    NoSelection,
    /// Token is valid
    TokenValid,
    /// Token is invalid
    TokenInvalid,
    /// Token stored successfully
    TokenStored,

    // === Error Response ===
    /// Error occurred (message is sanitized - no internal details)
    Error { message: String },

    // === Event Responses (after Subscribe) ===
    /// Real-time event
    Event { event: EventType },
}

/// Event types streamed to subscribed clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum EventType {
    /// Recording state changed
    StateChanged { state: RecordingState },
    /// Elapsed time update
    ElapsedTime { seconds: u64 },
    /// Transcoding started
    TranscodingStarted { format: String },
    /// Transcoding complete
    TranscodingComplete { success: bool, path: Option<String> },
    /// A transcription segment was produced
    TranscriptionSegment {
        /// Timestamp in seconds from recording start
        timestamp_secs: f64,
        /// The transcribed text
        text: String,
    },
    /// Service is shutting down
    Shutdown,
}

impl Response {
    /// Create an error response with a sanitized message.
    pub fn error(message: impl Into<String>) -> Self {
        Response::Error {
            message: message.into(),
        }
    }

    /// Create a success response.
    pub fn ok() -> Self {
        Response::Ok
    }

    /// Check if this response indicates an error.
    pub fn is_error(&self) -> bool {
        matches!(self, Response::Error { .. })
    }
}
