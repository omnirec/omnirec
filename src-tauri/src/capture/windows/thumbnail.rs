//! Windows thumbnail capture stub.
//!
//! This is a placeholder implementation that returns NotImplemented errors.
//! Full Windows thumbnail support will be implemented separately.

use crate::capture::error::CaptureError;
use crate::capture::{ThumbnailCapture, ThumbnailResult};

/// Windows thumbnail capture stub.
pub struct WindowsThumbnailCapture;

impl WindowsThumbnailCapture {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsThumbnailCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl ThumbnailCapture for WindowsThumbnailCapture {
    fn capture_window_thumbnail(&self, _window_handle: isize) -> Result<ThumbnailResult, CaptureError> {
        Err(CaptureError::NotImplemented(
            "Thumbnail capture not yet implemented on Windows".to_string(),
        ))
    }

    fn capture_display_thumbnail(&self, _monitor_id: &str) -> Result<ThumbnailResult, CaptureError> {
        Err(CaptureError::NotImplemented(
            "Thumbnail capture not yet implemented on Windows".to_string(),
        ))
    }

    fn capture_region_preview(
        &self,
        _monitor_id: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> Result<ThumbnailResult, CaptureError> {
        Err(CaptureError::NotImplemented(
            "Thumbnail capture not yet implemented on Windows".to_string(),
        ))
    }
}
