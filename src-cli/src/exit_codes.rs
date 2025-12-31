//! Exit codes for the CLI.
//!
//! These codes enable scripting integration by providing structured
//! feedback about operation results.

/// Exit codes for CLI operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ExitCode {
    /// Operation completed successfully
    Success = 0,
    /// General/unspecified error
    GeneralError = 1,
    /// Invalid command-line arguments
    InvalidArguments = 2,
    /// Failed to connect to the service
    ServiceConnectionFailed = 3,
    /// Recording failed to start
    RecordingFailedToStart = 4,
    /// Recording failed during capture
    RecordingFailedDuringCapture = 5,
    /// Transcoding failed (original MP4 preserved)
    TranscodingFailed = 6,
    /// Portal required (Wayland restriction with --strict)
    PortalRequired = 7,
    /// User cancelled (portal picker)
    UserCancelled = 8,
}

impl ExitCode {
    /// Convert to i32 for process exit.
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

impl std::fmt::Display for ExitCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExitCode::Success => write!(f, "success"),
            ExitCode::GeneralError => write!(f, "general error"),
            ExitCode::InvalidArguments => write!(f, "invalid arguments"),
            ExitCode::ServiceConnectionFailed => write!(f, "service connection failed"),
            ExitCode::RecordingFailedToStart => write!(f, "recording failed to start"),
            ExitCode::RecordingFailedDuringCapture => write!(f, "recording failed during capture"),
            ExitCode::TranscodingFailed => write!(f, "transcoding failed"),
            ExitCode::PortalRequired => write!(f, "portal required"),
            ExitCode::UserCancelled => write!(f, "user cancelled"),
        }
    }
}
