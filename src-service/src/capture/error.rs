//! Error types for capture operations.

use std::fmt;

/// Error type for capture operations.
#[derive(Debug)]
pub enum CaptureError {
    /// The requested capture target was not found
    TargetNotFound(String),
    /// Permission denied for capture
    PermissionDenied(String),
    /// Invalid capture parameters
    InvalidParameters(String),
    /// Invalid region specification
    InvalidRegion(String),
    /// Platform-specific capture error
    PlatformError(String),
    /// Feature not implemented on this platform
    NotImplemented(String),
    /// Capture was cancelled or stopped
    Cancelled,
    /// Audio-specific error
    AudioError(String),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CaptureError::TargetNotFound(msg) => write!(f, "Capture target not found: {}", msg),
            CaptureError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            CaptureError::InvalidParameters(msg) => write!(f, "Invalid parameters: {}", msg),
            CaptureError::InvalidRegion(msg) => write!(f, "Invalid region: {}", msg),
            CaptureError::PlatformError(msg) => write!(f, "Platform error: {}", msg),
            CaptureError::NotImplemented(msg) => write!(f, "Not implemented: {}", msg),
            CaptureError::Cancelled => write!(f, "Capture cancelled"),
            CaptureError::AudioError(msg) => write!(f, "Audio error: {}", msg),
        }
    }
}

impl std::error::Error for CaptureError {}

impl From<CaptureError> for String {
    fn from(err: CaptureError) -> Self {
        err.to_string()
    }
}

/// Error type for enumeration operations.
#[derive(Debug)]
pub enum EnumerationError {
    /// Platform-specific enumeration error
    PlatformError(String),
    /// Feature not implemented on this platform
    NotImplemented(String),
}

impl fmt::Display for EnumerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnumerationError::PlatformError(msg) => write!(f, "Enumeration error: {}", msg),
            EnumerationError::NotImplemented(msg) => write!(f, "Not implemented: {}", msg),
        }
    }
}

impl std::error::Error for EnumerationError {}

impl From<EnumerationError> for String {
    fn from(err: EnumerationError) -> Self {
        err.to_string()
    }
}
