//! macOS audio capture stub.
//!
//! Audio capture is not yet implemented for macOS.
//! This module provides stub implementations that return NotImplemented errors.

use crate::capture::error::{CaptureError, EnumerationError};
use crate::capture::types::{AudioReceiver, AudioSource, StopHandle};

/// List all available audio sources (stub - not implemented).
pub fn list_audio_sources() -> Result<Vec<AudioSource>, EnumerationError> {
    Err(EnumerationError::NotImplemented(
        "Audio capture not yet implemented for macOS".to_string(),
    ))
}

/// Start audio capture (stub - not implemented).
pub fn start_audio_capture(_source_id: &str) -> Result<(AudioReceiver, StopHandle), CaptureError> {
    Err(CaptureError::NotImplemented(
        "Audio capture not yet implemented for macOS".to_string(),
    ))
}

/// Initialize the audio backend (stub - no-op).
pub fn init_audio_backend() -> Result<(), String> {
    eprintln!("[Audio] macOS audio capture not yet implemented");
    Ok(())
}
