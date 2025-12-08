//! Windows platform capture implementation.

mod highlight;
mod monitor_list;
mod recorder;
mod region;
mod window_list;

use crate::capture::error::{CaptureError, EnumerationError};
use crate::capture::types::{CaptureRegion, FrameReceiver, MonitorInfo, StopHandle, WindowInfo};
use crate::capture::{CaptureBackend, HighlightProvider, MonitorEnumerator, WindowEnumerator};

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
