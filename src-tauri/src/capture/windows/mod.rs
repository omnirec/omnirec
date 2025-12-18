//! Windows platform capture implementation.

pub mod audio;
mod highlight;
mod monitor_list;
mod recorder;
mod region;
pub mod thumbnail;
mod window_list;

use crate::capture::error::{CaptureError, EnumerationError};
use crate::capture::types::{
    AudioReceiver, AudioSource, CaptureRegion, FrameReceiver, MonitorInfo, StopHandle, WindowInfo,
};
use crate::capture::{
    AudioCaptureBackend, AudioEnumerator, CaptureBackend, HighlightProvider, MonitorEnumerator,
    ThumbnailCapture, ThumbnailResult, WindowEnumerator,
};

/// Windows platform capture backend.
pub struct WindowsBackend;

impl WindowsBackend {
    /// Create a new Windows backend.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowEnumerator for WindowsBackend {
    fn list_windows(&self) -> Result<Vec<WindowInfo>, EnumerationError> {
        Ok(window_list::list_windows())
    }
}

impl MonitorEnumerator for WindowsBackend {
    fn list_monitors(&self) -> Result<Vec<MonitorInfo>, EnumerationError> {
        Ok(monitor_list::list_monitors())
    }
}

impl CaptureBackend for WindowsBackend {
    fn start_window_capture(
        &self,
        window_handle: isize,
    ) -> Result<(FrameReceiver, StopHandle), CaptureError> {
        recorder::start_capture(window_handle).map_err(|e| CaptureError::PlatformError(e))
    }

    fn start_region_capture(
        &self,
        region: CaptureRegion,
    ) -> Result<(FrameReceiver, StopHandle), CaptureError> {
        region::start_region_capture(region).map_err(|e| CaptureError::PlatformError(e))
    }

    fn start_display_capture(
        &self,
        monitor_id: String,
        width: u32,
        height: u32,
    ) -> Result<(FrameReceiver, StopHandle), CaptureError> {
        // Display capture is implemented as a full-monitor region capture
        let region = CaptureRegion {
            monitor_id,
            x: 0,
            y: 0,
            width,
            height,
        };
        region::start_region_capture(region).map_err(|e| CaptureError::PlatformError(e))
    }
}

impl HighlightProvider for WindowsBackend {
    fn show_highlight(&self, x: i32, y: i32, width: i32, height: i32) {
        highlight::show_highlight(x, y, width, height);
    }
}

impl ThumbnailCapture for WindowsBackend {
    fn capture_window_thumbnail(&self, window_handle: isize) -> Result<ThumbnailResult, CaptureError> {
        thumbnail::WindowsThumbnailCapture::new().capture_window_thumbnail(window_handle)
    }

    fn capture_display_thumbnail(&self, monitor_id: &str) -> Result<ThumbnailResult, CaptureError> {
        thumbnail::WindowsThumbnailCapture::new().capture_display_thumbnail(monitor_id)
    }

    fn capture_region_preview(
        &self,
        monitor_id: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<ThumbnailResult, CaptureError> {
        thumbnail::WindowsThumbnailCapture::new().capture_region_preview(monitor_id, x, y, width, height)
    }
}

impl AudioEnumerator for WindowsBackend {
    fn list_audio_sources(&self) -> Result<Vec<AudioSource>, EnumerationError> {
        audio::list_audio_sources()
    }
}

impl AudioCaptureBackend for WindowsBackend {
    fn start_audio_capture(
        &self,
        source_id: &str,
    ) -> Result<(AudioReceiver, StopHandle), CaptureError> {
        audio::start_audio_capture(source_id)
    }
}

// Extension methods for cross-platform API compatibility
impl WindowsBackend {
    /// Start audio capture from up to two sources with optional AEC.
    ///
    /// Supports:
    /// - System audio only (loopback capture)
    /// - Microphone only (direct capture)
    /// - Both sources with mixing and optional acoustic echo cancellation (AEC)
    pub fn start_audio_capture_dual(
        &self,
        system_source_id: Option<&str>,
        mic_source_id: Option<&str>,
        aec_enabled: bool,
    ) -> Result<(AudioReceiver, StopHandle), CaptureError> {
        // If only one source is specified, use the simpler single-source capture
        match (system_source_id, mic_source_id) {
            (Some(sys_id), None) => audio::start_audio_capture(sys_id),
            (None, Some(mic_id)) => audio::start_audio_capture(mic_id),
            (Some(_), Some(_)) => {
                // Both sources - use dual capture with mixing
                audio::start_audio_capture_dual(system_source_id, mic_source_id, aec_enabled)
            }
            (None, None) => Err(CaptureError::AudioError(
                "No audio source specified".to_string(),
            )),
        }
    }
}

/// Initialize the audio capture subsystem.
pub fn init_audio() -> Result<(), String> {
    audio::init_audio_backend()
}
