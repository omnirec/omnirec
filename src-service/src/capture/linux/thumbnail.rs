//! Linux thumbnail capture implementation using wlr-screencopy.
//!
//! This module captures single frames from outputs for use as thumbnails in the UI.
//! It uses the wlr-screencopy protocol for fast, efficient capture without the
//! overhead of portal/PipeWire infrastructure.

use crate::capture::error::CaptureError;
use crate::capture::thumbnail::{
    bgra_to_jpeg_thumbnail, PREVIEW_MAX_HEIGHT, PREVIEW_MAX_WIDTH, THUMBNAIL_MAX_HEIGHT,
    THUMBNAIL_MAX_WIDTH,
};
use crate::capture::{ThumbnailCapture, ThumbnailResult};

use super::screencopy;

use hyprland::data::{Clients, Monitors};
use hyprland::shared::HyprData;

/// Crop a BGRA frame to a specified region.
fn crop_frame(
    data: &[u8],
    frame_width: u32,
    frame_height: u32,
    crop_x: i32,
    crop_y: i32,
    crop_width: u32,
    crop_height: u32,
) -> Vec<u8> {
    // Clamp crop region to frame bounds
    let x = crop_x.max(0) as u32;
    let y = crop_y.max(0) as u32;
    let w = crop_width.min(frame_width.saturating_sub(x));
    let h = crop_height.min(frame_height.saturating_sub(y));
    
    if w == 0 || h == 0 {
        return Vec::new();
    }
    
    let mut cropped = vec![0u8; (w * h * 4) as usize];
    
    for row in 0..h {
        let src_offset = ((y + row) * frame_width + x) as usize * 4;
        let dst_offset = (row * w) as usize * 4;
        let row_bytes = (w * 4) as usize;
        
        if src_offset + row_bytes <= data.len() {
            cropped[dst_offset..dst_offset + row_bytes]
                .copy_from_slice(&data[src_offset..src_offset + row_bytes]);
        }
    }
    
    cropped
}

/// Find which monitor contains a window based on its position.
fn find_monitor_for_window(window_x: i32, window_y: i32) -> Option<String> {
    let monitors = Monitors::get().ok()?;
    
    for monitor in monitors.iter() {
        let mon_x = monitor.x;
        let mon_y = monitor.y;
        // Use logical dimensions for comparison (window coords are logical)
        let mon_width = (monitor.width as f64 / monitor.scale as f64).round() as i32;
        let mon_height = (monitor.height as f64 / monitor.scale as f64).round() as i32;
        
        if window_x >= mon_x && window_x < mon_x + mon_width
            && window_y >= mon_y && window_y < mon_y + mon_height
        {
            return Some(monitor.name.clone());
        }
    }
    
    // Fallback to first monitor if window position doesn't match any
    monitors.iter().next().map(|m| m.name.clone())
}

/// Get monitor info by name.
fn get_monitor_info(monitor_name: &str) -> Option<(i32, i32, u32, u32, f64)> {
    let monitors = Monitors::get().ok()?;
    monitors.iter()
        .find(|m| m.name == monitor_name)
        .map(|m| (m.x, m.y, m.width as u32, m.height as u32, m.scale as f64))
}

/// Linux thumbnail capture implementation using wlr-screencopy.
pub struct LinuxThumbnailCapture;

impl LinuxThumbnailCapture {
    /// Create a new Linux thumbnail capture instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxThumbnailCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl ThumbnailCapture for LinuxThumbnailCapture {
    fn capture_window_thumbnail(&self, window_handle: isize) -> Result<ThumbnailResult, CaptureError> {
        // Get window info from Hyprland
        let clients = Clients::get().map_err(|e| {
            CaptureError::PlatformError(format!("Failed to get Hyprland clients: {}", e))
        })?;
        
        // Convert handle to hex address for comparison
        let target_address = format!("0x{:x}", window_handle as usize);
        
        let client = clients.iter()
            .find(|c| {
                let addr = c.address.to_string();
                addr == target_address || addr.trim_start_matches("0x") == target_address.trim_start_matches("0x")
            })
            .ok_or_else(|| {
                CaptureError::TargetNotFound(format!("Window with handle {} not found", window_handle))
            })?;
        
        // Find which monitor contains this window
        let monitor_name = find_monitor_for_window(client.at.0 as i32, client.at.1 as i32)
            .ok_or_else(|| CaptureError::PlatformError("Could not find monitor for window".to_string()))?;
        
        // Get monitor info for coordinate conversion
        let (mon_x, mon_y, _mon_width, _mon_height, scale) = get_monitor_info(&monitor_name)
            .ok_or_else(|| CaptureError::PlatformError(format!("Monitor '{}' not found", monitor_name)))?;
        
        // Capture the output
        let frame = screencopy::capture_output(&monitor_name)
            .map_err(CaptureError::PlatformError)?;
        
        // Calculate window position relative to monitor in physical pixels
        // Window coordinates from Hyprland are in logical space
        let window_x_logical = client.at.0 as i32 - mon_x;
        let window_y_logical = client.at.1 as i32 - mon_y;
        let window_width_logical = client.size.0 as u32;
        let window_height_logical = client.size.1 as u32;
        
        // Convert to physical pixels for cropping the captured frame
        let crop_x = (window_x_logical as f64 * scale).round() as i32;
        let crop_y = (window_y_logical as f64 * scale).round() as i32;
        let crop_width = (window_width_logical as f64 * scale).round() as u32;
        let crop_height = (window_height_logical as f64 * scale).round() as u32;
        
        // Crop the frame to window bounds
        let cropped = crop_frame(
            &frame.data,
            frame.width,
            frame.height,
            crop_x,
            crop_y,
            crop_width,
            crop_height,
        );
        
        if cropped.is_empty() {
            return Err(CaptureError::PlatformError("Crop resulted in empty frame".to_string()));
        }
        
        // Convert to thumbnail
        let (base64_data, thumb_width, thumb_height) = bgra_to_jpeg_thumbnail(
            &cropped,
            crop_width,
            crop_height,
            THUMBNAIL_MAX_WIDTH,
            THUMBNAIL_MAX_HEIGHT,
        )
        .map_err(CaptureError::PlatformError)?;
        
        Ok(ThumbnailResult {
            data: base64_data,
            width: thumb_width,
            height: thumb_height,
        })
    }

    fn capture_display_thumbnail(&self, monitor_id: &str) -> Result<ThumbnailResult, CaptureError> {
        // Capture the output directly via screencopy
        let frame = screencopy::capture_output(monitor_id)
            .map_err(CaptureError::PlatformError)?;

        // Convert to thumbnail
        let (base64_data, thumb_width, thumb_height) = bgra_to_jpeg_thumbnail(
            &frame.data,
            frame.width,
            frame.height,
            THUMBNAIL_MAX_WIDTH,
            THUMBNAIL_MAX_HEIGHT,
        )
        .map_err(CaptureError::PlatformError)?;

        Ok(ThumbnailResult {
            data: base64_data,
            width: thumb_width,
            height: thumb_height,
        })
    }

    fn capture_region_preview(
        &self,
        monitor_id: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<ThumbnailResult, CaptureError> {

        // Validate region
        if width < 100 || height < 100 {
            return Err(CaptureError::InvalidRegion(format!(
                "Region must be at least 100x100 pixels (got {}x{})",
                width, height
            )));
        }

        // Get monitor info including scale factor
        let (_, _, _, _, scale) = get_monitor_info(monitor_id)
            .ok_or_else(|| CaptureError::TargetNotFound(format!("Monitor '{}' not found", monitor_id)))?;

        // Capture the output via screencopy (returns physical pixels)
        let frame = screencopy::capture_output(monitor_id)
            .map_err(CaptureError::PlatformError)?;

        // Region coordinates from frontend are in LOGICAL pixels (from Hyprland)
        // Screencopy capture is in PHYSICAL pixels
        // Need to scale the crop coordinates
        let crop_x = (x as f64 * scale).round() as i32;
        let crop_y = (y as f64 * scale).round() as i32;
        let crop_width = (width as f64 * scale).round() as u32;
        let crop_height = (height as f64 * scale).round() as u32;

        // Crop the frame to region bounds
        let cropped = crop_frame(
            &frame.data,
            frame.width,
            frame.height,
            crop_x,
            crop_y,
            crop_width,
            crop_height,
        );

        if cropped.is_empty() {
            return Err(CaptureError::PlatformError("Crop resulted in empty frame".to_string()));
        }

        // Convert to preview (larger than thumbnail)
        let (base64_data, preview_width, preview_height) = bgra_to_jpeg_thumbnail(
            &cropped,
            crop_width,
            crop_height,
            PREVIEW_MAX_WIDTH,
            PREVIEW_MAX_HEIGHT,
        )
        .map_err(CaptureError::PlatformError)?;

        Ok(ThumbnailResult {
            data: base64_data,
            width: preview_width,
            height: preview_height,
        })
    }
}
