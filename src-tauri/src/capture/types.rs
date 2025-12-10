//! Shared types for cross-platform capture operations.

use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Information about a capturable window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    /// Window handle/ID as an integer (platform-specific)
    pub handle: isize,
    /// Window title
    pub title: String,
    /// Process name (executable name)
    pub process_name: String,
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

/// A captured frame with its dimensions and pixel data.
#[derive(Clone)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    /// BGRA pixel data
    pub data: Vec<u8>,
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

/// Handle to stop an ongoing capture.
pub type StopHandle = Arc<AtomicBool>;

/// Receiver for captured frames.
pub type FrameReceiver = mpsc::Receiver<CapturedFrame>;
