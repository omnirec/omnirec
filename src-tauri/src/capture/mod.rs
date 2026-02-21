//! Cross-platform capture module for the OmniRec service.
//!
//! This module provides platform-agnostic interfaces for screen capture operations,
//! with platform-specific implementations selected at compile time.

// Allow dead code and unused imports during migration - not all capture functionality is wired up yet
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod error;
pub mod thumbnail;
pub mod types;

// Platform-specific modules
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

// Re-export common types from omnirec-common for IPC
pub use omnirec_common::{
    AudioConfig, AudioSource, AudioSourceType, CaptureRegion, MonitorInfo, OutputFormat,
    RecordingState, WindowInfo,
};

// Re-export local error types
pub use error::{CaptureError, EnumerationError};

// Re-export runtime types (service-internal, not for IPC)
pub use types::{AudioReceiver, AudioSample, CapturedFrame, FrameReceiver, StopHandle};

// Re-export thumbnail utilities (used by platform implementations)
#[allow(unused_imports)]
pub use thumbnail::{
    bgra_to_jpeg_thumbnail, PREVIEW_MAX_HEIGHT, PREVIEW_MAX_WIDTH, THUMBNAIL_MAX_HEIGHT,
    THUMBNAIL_MAX_WIDTH,
};

// Platform-specific backend aliases
#[cfg(target_os = "linux")]
pub use linux::LinuxBackend as PlatformBackend;
#[cfg(target_os = "macos")]
pub use macos::MacOSBackend as PlatformBackend;
#[cfg(target_os = "windows")]
pub use windows::WindowsBackend as PlatformBackend;

/// Trait for window enumeration operations.
pub trait WindowEnumerator: Send + Sync {
    /// List all visible, capturable windows.
    fn list_windows(&self) -> Result<Vec<WindowInfo>, EnumerationError>;
}

/// Trait for monitor/display enumeration operations.
pub trait MonitorEnumerator: Send + Sync {
    /// List all connected monitors.
    fn list_monitors(&self) -> Result<Vec<MonitorInfo>, EnumerationError>;
}

/// Trait for capture operations.
#[allow(dead_code)]
pub trait CaptureBackend: Send + Sync {
    /// Start capturing a window by its handle/ID.
    ///
    /// Returns a frame receiver and stop handle.
    fn start_window_capture(
        &self,
        window_handle: isize,
    ) -> Result<(FrameReceiver, StopHandle), CaptureError>;

    /// Start capturing a screen region.
    ///
    /// Returns a frame receiver and stop handle.
    fn start_region_capture(
        &self,
        region: CaptureRegion,
    ) -> Result<(FrameReceiver, StopHandle), CaptureError>;

    /// Start capturing an entire display.
    ///
    /// Returns a frame receiver and stop handle.
    fn start_display_capture(
        &self,
        monitor_id: String,
        width: u32,
        height: u32,
    ) -> Result<(FrameReceiver, StopHandle), CaptureError>;
}

/// Trait for visual highlight rendering.
pub trait HighlightProvider: Send + Sync {
    /// Show a highlight border around the specified area.
    fn show_highlight(&self, x: i32, y: i32, width: i32, height: i32);
}

/// Trait for audio device enumeration operations.
pub trait AudioEnumerator: Send + Sync {
    /// List all available audio sources (inputs and output monitors).
    fn list_audio_sources(&self) -> Result<Vec<AudioSource>, EnumerationError>;
}

/// Trait for audio capture operations.
#[allow(dead_code)]
pub trait AudioCaptureBackend: Send + Sync {
    /// Start capturing audio from the specified source.
    ///
    /// Returns an audio sample receiver and stop handle.
    /// Audio is captured as 48kHz stereo f32 samples.
    fn start_audio_capture(
        &self,
        source_id: &str,
    ) -> Result<(AudioReceiver, StopHandle), CaptureError>;
}

/// Result of a thumbnail capture operation.
#[derive(Debug, Clone)]
pub struct ThumbnailResult {
    /// Base64-encoded JPEG image data
    pub data: String,
    /// Thumbnail width in pixels
    pub width: u32,
    /// Thumbnail height in pixels
    pub height: u32,
}

/// Trait for thumbnail capture operations.
pub trait ThumbnailCapture: Send + Sync {
    /// Capture a thumbnail of a window.
    ///
    /// Returns a base64-encoded JPEG image scaled to fit within max dimensions.
    fn capture_window_thumbnail(
        &self,
        window_handle: isize,
    ) -> Result<ThumbnailResult, CaptureError>;

    /// Capture a thumbnail of a display.
    ///
    /// Returns a base64-encoded JPEG image scaled to fit within max dimensions.
    fn capture_display_thumbnail(&self, monitor_id: &str) -> Result<ThumbnailResult, CaptureError>;

    /// Capture a preview of a screen region.
    ///
    /// Returns a base64-encoded JPEG image of the specified region.
    fn capture_region_preview(
        &self,
        monitor_id: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<ThumbnailResult, CaptureError>;
}

/// Get the platform-specific capture backend.
pub fn get_backend() -> PlatformBackend {
    PlatformBackend::new()
}

// Convenience functions that use the platform backend

/// List all visible, capturable windows.
pub fn list_windows() -> Vec<WindowInfo> {
    let backend = get_backend();
    backend.list_windows().unwrap_or_default()
}

/// List all connected monitors.
pub fn list_monitors() -> Vec<MonitorInfo> {
    let backend = get_backend();
    backend.list_monitors().unwrap_or_default()
}

/// Show a highlight border around the specified area.
pub fn show_highlight(x: i32, y: i32, width: i32, height: i32) {
    let backend = get_backend();
    backend.show_highlight(x, y, width, height);
}

/// List all available audio sources.
pub fn list_audio_sources() -> Vec<AudioSource> {
    let backend = get_backend();
    backend.list_audio_sources().unwrap_or_default()
}
