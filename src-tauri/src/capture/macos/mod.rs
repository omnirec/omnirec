//! macOS platform capture implementation using ScreenCaptureKit.
//!
//! This module provides screen capture functionality on macOS through:
//! - Core Graphics for window/display enumeration
//! - ScreenCaptureKit for high-performance frame capture
//! - TCC for permission handling

pub mod audio;
pub mod highlight;
pub mod monitor_list;
pub mod recorder;
pub mod thumbnail;
pub mod window_list;

use crate::capture::error::{CaptureError, EnumerationError};
use crate::capture::types::{
    AudioReceiver, AudioSource, CaptureRegion, CapturedFrame, FrameReceiver, MonitorInfo,
    StopHandle, WindowInfo,
};
use crate::capture::{
    AudioCaptureBackend, AudioEnumerator, CaptureBackend, HighlightProvider, MonitorEnumerator,
    ThumbnailCapture, ThumbnailResult, WindowEnumerator,
};
use std::sync::atomic::Ordering;
use tokio::sync::mpsc;

/// macOS platform capture backend using ScreenCaptureKit.
pub struct MacOSBackend;

impl MacOSBackend {
    /// Create a new macOS backend.
    pub fn new() -> Self {
        Self
    }

    /// Check if running on a supported macOS version (12.3+).
    #[allow(dead_code)]
    pub fn is_supported_version() -> bool {
        // ScreenCaptureKit requires macOS 12.3+
        // We check by attempting to access the SCK types
        // For now, assume supported if we're compiling for macOS
        true
    }

    /// Check if screen recording permission is granted.
    pub fn has_screen_recording_permission() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    /// Request screen recording permission from the user.
    ///
    /// This will show the system permission prompt if permission hasn't been determined yet.
    /// Returns true if permission is granted, false otherwise.
    pub fn request_screen_recording_permission() -> bool {
        unsafe { CGRequestScreenCaptureAccess() }
    }

    /// Trigger the app to be added to the Screen Recording permission list.
    ///
    /// This calls CGRequestScreenCaptureAccess which will cause macOS to
    /// show the permission prompt (first time only) and add the app to the
    /// Screen Recording list in System Settings.
    pub fn trigger_permission_prompt() {
        // CGRequestScreenCaptureAccess is the official API to trigger the permission prompt
        // It returns immediately and shows the prompt asynchronously
        unsafe { CGRequestScreenCaptureAccess() };
    }

    /// Check and request permission, returning an error if not granted.
    fn ensure_permission() -> Result<(), CaptureError> {
        if !Self::has_screen_recording_permission()
            && !Self::request_screen_recording_permission()
        {
            return Err(CaptureError::PermissionDenied(
                "Screen recording permission required. Please grant permission in System Settings > Privacy & Security > Screen Recording, then restart the app.".to_string()
            ));
        }
        Ok(())
    }
}

impl Default for MacOSBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowEnumerator for MacOSBackend {
    fn list_windows(&self) -> Result<Vec<WindowInfo>, EnumerationError> {
        Ok(window_list::list_windows())
    }
}

impl MonitorEnumerator for MacOSBackend {
    fn list_monitors(&self) -> Result<Vec<MonitorInfo>, EnumerationError> {
        Ok(monitor_list::list_monitors())
    }
}

impl CaptureBackend for MacOSBackend {
    fn start_window_capture(
        &self,
        window_handle: isize,
    ) -> Result<(FrameReceiver, StopHandle), CaptureError> {
        Self::ensure_permission()?;

        // Convert handle to CGWindowID (u32)
        let window_id = window_handle as u32;

        recorder::start_window_capture(window_id).map_err(CaptureError::PlatformError)
    }

    fn start_region_capture(
        &self,
        region: CaptureRegion,
    ) -> Result<(FrameReceiver, StopHandle), CaptureError> {
        Self::ensure_permission()?;

        // Validate region
        if region.width < 100 || region.height < 100 {
            return Err(CaptureError::InvalidRegion(format!(
                "Region must be at least 100x100 pixels (got {}x{})",
                region.width, region.height
            )));
        }

        // Parse monitor ID as display ID
        let display_id: u32 = region
            .monitor_id
            .parse()
            .map_err(|_| CaptureError::InvalidParameters("Invalid monitor ID".to_string()))?;

        // Start display capture first
        let monitors = monitor_list::list_monitors();
        let monitor = monitors
            .iter()
            .find(|m| m.id == region.monitor_id)
            .ok_or_else(|| {
                CaptureError::TargetNotFound(format!("Monitor {} not found", region.monitor_id))
            })?;

        // Get scale factor for coordinate conversion
        let scale = monitor.scale_factor;
        
        // Calculate physical dimensions for capture
        // monitor.width/height are logical, need physical for ScreenCaptureKit
        let physical_monitor_width = ((monitor.width as f64) * scale).round() as u32;
        let physical_monitor_height = ((monitor.height as f64) * scale).round() as u32;

        // Start capturing the full display at physical resolution
        let (mut display_rx, stop_handle) =
            recorder::start_display_capture(display_id, physical_monitor_width, physical_monitor_height)
                .map_err(CaptureError::PlatformError)?;

        // Create a new channel for cropped frames
        let (tx, rx) = mpsc::channel(3);

        eprintln!("[macOS] === REGION CAPTURE DEBUG ===");
        eprintln!("[macOS] Input region from frontend (logical coords):");
        eprintln!("[macOS]   x={}, y={}, width={}, height={}", 
            region.x, region.y, region.width, region.height);
        eprintln!("[macOS] Monitor info:");
        eprintln!("[macOS]   id={}, logical_origin=({}, {}), logical_size={}x{}, scale={}", 
            monitor.id, monitor.x, monitor.y, monitor.width, monitor.height, scale);
        eprintln!("[macOS]   physical_size={}x{}", physical_monitor_width, physical_monitor_height);
        
        // Region from frontend is in logical coordinates (matching monitor coordinate system)
        // Need to convert to physical pixels for cropping captured frames
        let region_x_physical = ((region.x.max(0) as f64) * scale).round() as u32;
        let region_y_physical = ((region.y.max(0) as f64) * scale).round() as u32;
        let region_width_physical = ((region.width as f64) * scale).round() as u32;
        let region_height_physical = ((region.height as f64) * scale).round() as u32;

        // Clamp to physical monitor bounds
        let max_width = physical_monitor_width.saturating_sub(region_x_physical);
        let max_height = physical_monitor_height.saturating_sub(region_y_physical);
        let region_x = region_x_physical;
        let region_y = region_y_physical;
        let region_width = region_width_physical.min(max_width);
        let region_height = region_height_physical.min(max_height);

        // Validate we still have a valid region (in physical pixels)
        let min_size_physical = ((100.0) * scale).round() as u32;
        if region_width < min_size_physical || region_height < min_size_physical {
            return Err(CaptureError::InvalidRegion(format!(
                "Region too small after scaling to physical pixels ({}x{}, need {}x{})",
                region_width, region_height, min_size_physical, min_size_physical
            )));
        }

        eprintln!("[macOS] Final crop region (physical pixels):");
        eprintln!("[macOS]   x={}, y={}, width={}, height={}", 
            region_x, region_y, region_width, region_height);
        eprintln!("[macOS] =============================");

        let stop_flag = stop_handle.clone();

        // Spawn a task to crop frames
        tokio::spawn(async move {
            eprintln!("[macOS] Cropping task started");
            let mut frame_count = 0u32;
            loop {
                // Check stop flag before waiting for frame
                if stop_flag.load(Ordering::Relaxed) {
                    eprintln!("[macOS] Cropping task: stop flag set, exiting");
                    break;
                }

                // Use timeout to periodically check stop flag
                match tokio::time::timeout(
                    std::time::Duration::from_millis(100),
                    display_rx.recv()
                ).await {
                    Ok(Some(frame)) => {
                        // Log first frame info
                        if frame_count == 0 {
                            eprintln!("[macOS] First frame received: {}x{}", frame.width, frame.height);
                            eprintln!("[macOS] Cropping to: {}x{} at ({},{})", 
                                region_width, region_height, region_x, region_y);
                        }
                        frame_count += 1;
                        
                        // Crop the frame to the region
                        if let Some(cropped) = crop_frame(
                            &frame,
                            region_x,
                            region_y,
                            region_width,
                            region_height,
                        ) {
                            if tx.send(cropped).await.is_err() {
                                eprintln!("[macOS] Cropping task: send failed, exiting");
                                break;
                            }
                        }
                    }
                    Ok(None) => {
                        // Channel closed
                        eprintln!("[macOS] Cropping task: channel closed, exiting");
                        break;
                    }
                    Err(_) => {
                        // Timeout - check stop flag on next iteration
                        continue;
                    }
                }
            }
            eprintln!("[macOS] Cropping task finished");
        });

        Ok((rx, stop_handle))
    }

    fn start_display_capture(
        &self,
        monitor_id: String,
        width: u32,
        height: u32,
    ) -> Result<(FrameReceiver, StopHandle), CaptureError> {
        Self::ensure_permission()?;

        // Parse monitor ID as display ID
        let display_id: u32 = monitor_id
            .parse()
            .map_err(|_| CaptureError::InvalidParameters("Invalid monitor ID".to_string()))?;

        recorder::start_display_capture(display_id, width, height)
            .map_err(CaptureError::PlatformError)
    }
}

impl HighlightProvider for MacOSBackend {
    fn show_highlight(&self, x: i32, y: i32, width: i32, height: i32) {
        highlight::show_highlight(x, y, width, height);
    }
}

impl ThumbnailCapture for MacOSBackend {
    fn capture_window_thumbnail(&self, window_handle: isize) -> Result<ThumbnailResult, CaptureError> {
        thumbnail::MacOSThumbnailCapture::new().capture_window_thumbnail(window_handle)
    }

    fn capture_display_thumbnail(&self, monitor_id: &str) -> Result<ThumbnailResult, CaptureError> {
        thumbnail::MacOSThumbnailCapture::new().capture_display_thumbnail(monitor_id)
    }

    fn capture_region_preview(
        &self,
        monitor_id: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<ThumbnailResult, CaptureError> {
        thumbnail::MacOSThumbnailCapture::new().capture_region_preview(monitor_id, x, y, width, height)
    }
}

impl AudioEnumerator for MacOSBackend {
    fn list_audio_sources(&self) -> Result<Vec<AudioSource>, EnumerationError> {
        audio::list_audio_sources()
    }
}

impl AudioCaptureBackend for MacOSBackend {
    fn start_audio_capture(
        &self,
        source_id: &str,
    ) -> Result<(AudioReceiver, StopHandle), CaptureError> {
        audio::start_audio_capture(source_id)
    }
}

// Extension methods for cross-platform API compatibility
impl MacOSBackend {
    /// Start audio capture from up to two sources with optional AEC.
    ///
    /// Note: Dual-source mixing with AEC is currently only implemented on Linux.
    /// On macOS, this falls back to capturing only the system audio source.
    pub fn start_audio_capture_dual(
        &self,
        system_source_id: Option<&str>,
        _mic_source_id: Option<&str>,
        _aec_enabled: bool,
    ) -> Result<(AudioReceiver, StopHandle), CaptureError> {
        // TODO: Implement dual-source mixing with AEC on macOS
        // For now, just capture the system audio source
        if let Some(source_id) = system_source_id {
            audio::start_audio_capture(source_id)
        } else {
            Err(CaptureError::AudioError(
                "No audio source specified".to_string(),
            ))
        }
    }
}

/// Initialize the audio capture subsystem.
pub fn init_audio() -> Result<(), String> {
    audio::init_audio_backend()
}

/// Crop a captured frame to a specified region.
fn crop_frame(
    frame: &CapturedFrame,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Option<CapturedFrame> {
    // Validate crop region fits within frame
    if x + width > frame.width || y + height > frame.height {
        eprintln!(
            "[macOS] Crop region {}x{} at ({},{}) exceeds frame size {}x{}",
            width, height, x, y, frame.width, frame.height
        );
        return None;
    }

    // Create cropped buffer (4 bytes per pixel for BGRA)
    let mut cropped_data = Vec::with_capacity((width * height * 4) as usize);

    let src_stride = (frame.width * 4) as usize;
    let dst_stride = (width * 4) as usize;

    for row in 0..height {
        let src_y = (y + row) as usize;
        let src_start = src_y * src_stride + (x * 4) as usize;
        let src_end = src_start + dst_stride;

        if src_end <= frame.data.len() {
            cropped_data.extend_from_slice(&frame.data[src_start..src_end]);
        }
    }

    Some(CapturedFrame {
        width,
        height,
        data: cropped_data,
    })
}

// External declarations for Core Graphics permission functions
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_creation() {
        let _backend = MacOSBackend::new();
    }

    #[test]
    fn test_is_supported_version() {
        // Should be true on macOS builds
        assert!(MacOSBackend::is_supported_version());
    }

    #[test]
    fn test_crop_frame() {
        // Create a test frame 10x10 pixels (400 bytes for BGRA)
        let frame = CapturedFrame {
            width: 10,
            height: 10,
            data: vec![0u8; 400],
        };

        // Crop to 5x5 at (2,2)
        let cropped = crop_frame(&frame, 2, 2, 5, 5);
        assert!(cropped.is_some());

        let cropped = cropped.unwrap();
        assert_eq!(cropped.width, 5);
        assert_eq!(cropped.height, 5);
        assert_eq!(cropped.data.len(), 100); // 5*5*4
    }

    #[test]
    fn test_crop_frame_invalid() {
        let frame = CapturedFrame {
            width: 10,
            height: 10,
            data: vec![0u8; 400],
        };

        // Try to crop beyond bounds
        let cropped = crop_frame(&frame, 8, 8, 5, 5);
        assert!(cropped.is_none());
    }
}
