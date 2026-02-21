//! macOS thumbnail capture implementation using Core Graphics.
//!
//! This module captures single frames from displays and windows for use as
//! thumbnails in the UI. It uses Core Graphics APIs for immediate capture
//! without the overhead of ScreenCaptureKit streaming.

use crate::capture::error::CaptureError;
use crate::capture::thumbnail::{
    bgra_to_jpeg_thumbnail, PREVIEW_MAX_HEIGHT, PREVIEW_MAX_WIDTH, THUMBNAIL_MAX_HEIGHT,
    THUMBNAIL_MAX_WIDTH,
};
use crate::capture::{ThumbnailCapture, ThumbnailResult};

use super::monitor_list;

use core_graphics::display::{
    kCGWindowImageBoundsIgnoreFraming, kCGWindowListOptionIncludingWindow, CGDirectDisplayID,
    CGDisplay, CGRect, CGWindowID, CGWindowListCreateImage,
};
use core_graphics::image::CGImage;
use foreign_types::ForeignType;

// External declaration for permission check
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
}

/// Check if screen recording permission is granted.
fn has_permission() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() }
}

/// Ensure screen recording permission is granted.
fn ensure_permission() -> Result<(), CaptureError> {
    if !has_permission() {
        return Err(CaptureError::PermissionDenied(
            "Screen recording permission required for thumbnail capture. Please grant permission in System Settings > Privacy & Security > Screen Recording, then restart the app.".to_string()
        ));
    }
    Ok(())
}

/// Convert a CGImage to BGRA pixel data.
fn cgimage_to_bgra(image: &CGImage) -> Result<(Vec<u8>, u32, u32), String> {
    let width = image.width() as u32;
    let height = image.height() as u32;
    let bytes_per_row = image.bytes_per_row();
    let bits_per_pixel = image.bits_per_pixel();

    // Get the raw pixel data using the data() method
    let raw_data = image.data();

    // CGImage typically returns data in BGRA or RGBA format depending on the source
    // We need to handle the byte order correctly
    let bytes_per_pixel = bits_per_pixel / 8;

    if bytes_per_pixel != 4 {
        return Err(format!(
            "Unexpected bits per pixel: {} (expected 32)",
            bits_per_pixel
        ));
    }

    // Copy data, handling row padding if present
    let expected_stride = (width as usize) * 4;
    let mut bgra_data = Vec::with_capacity((width * height * 4) as usize);

    for row in 0..height as usize {
        let src_offset = row * bytes_per_row;
        let src_end = src_offset + expected_stride;

        if src_end <= raw_data.len() as usize {
            // CGImage on macOS uses BGRA format (kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little)
            // which is already BGRA, but we need to verify and potentially swap
            let row_data = &raw_data[src_offset..src_end];

            // The data is typically in BGRA format on macOS
            bgra_data.extend_from_slice(row_data);
        }
    }

    Ok((bgra_data, width, height))
}

/// Capture a display to CGImage.
fn capture_display(display_id: CGDirectDisplayID) -> Result<CGImage, CaptureError> {
    let display = CGDisplay::new(display_id);

    // CGDisplayCreateImage captures the entire display
    let image = display.image().ok_or_else(|| {
        CaptureError::PlatformError(format!(
            "Failed to capture display {} - CGDisplayCreateImage returned null",
            display_id
        ))
    })?;

    Ok(image)
}

/// Capture a window to CGImage.
fn capture_window(window_id: CGWindowID) -> Result<CGImage, CaptureError> {
    // Use CGWindowListCreateImage to capture a specific window
    // CGRectNull means capture the window's bounds
    let cg_rect_null = CGRect::new(
        &core_graphics::geometry::CGPoint::new(0.0, 0.0),
        &core_graphics::geometry::CGSize::new(0.0, 0.0),
    );

    let image_ref = unsafe {
        CGWindowListCreateImage(
            cg_rect_null,
            kCGWindowListOptionIncludingWindow,
            window_id,
            kCGWindowImageBoundsIgnoreFraming,
        )
    };

    if image_ref.is_null() {
        return Err(CaptureError::TargetNotFound(format!(
            "Failed to capture window {} - window may be closed or minimized",
            window_id
        )));
    }

    // Convert raw pointer to CGImage
    // Safety: CGWindowListCreateImage returns a CGImageRef that we own
    let image = unsafe { CGImage::from_ptr(image_ref) };

    Ok(image)
}

/// Get the scale factor for a monitor by its ID.
fn get_monitor_scale_factor(monitor_id: &str) -> f64 {
    let monitors = monitor_list::list_monitors();
    monitors
        .iter()
        .find(|m| m.id == monitor_id)
        .map(|m| m.scale_factor)
        .unwrap_or(1.0)
}

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
    let src_stride = frame_width as usize * 4;
    let dst_stride = w as usize * 4;

    for row in 0..h {
        let src_offset = ((y + row) as usize * src_stride) + (x as usize * 4);
        let dst_offset = row as usize * dst_stride;

        if src_offset + dst_stride <= data.len() {
            cropped[dst_offset..dst_offset + dst_stride]
                .copy_from_slice(&data[src_offset..src_offset + dst_stride]);
        }
    }

    cropped
}

/// macOS thumbnail capture implementation using Core Graphics.
pub struct MacOSThumbnailCapture;

impl MacOSThumbnailCapture {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOSThumbnailCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl ThumbnailCapture for MacOSThumbnailCapture {
    fn capture_window_thumbnail(
        &self,
        window_handle: isize,
    ) -> Result<ThumbnailResult, CaptureError> {
        // Check permission first
        ensure_permission()?;

        // Convert handle to CGWindowID
        let window_id = window_handle as CGWindowID;

        // Capture the window
        let image = capture_window(window_id)?;

        // Convert CGImage to BGRA data
        let (bgra_data, width, height) =
            cgimage_to_bgra(&image).map_err(CaptureError::PlatformError)?;

        // Convert to thumbnail
        let (base64_data, thumb_width, thumb_height) = bgra_to_jpeg_thumbnail(
            &bgra_data,
            width,
            height,
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
        // Check permission first
        ensure_permission()?;

        // Parse monitor ID as display ID
        let display_id: CGDirectDisplayID = monitor_id.parse().map_err(|_| {
            CaptureError::InvalidParameters(format!("Invalid monitor ID: {}", monitor_id))
        })?;

        // Capture the display
        let image = capture_display(display_id)?;

        // Convert CGImage to BGRA data
        let (bgra_data, width, height) =
            cgimage_to_bgra(&image).map_err(CaptureError::PlatformError)?;

        // Convert to thumbnail
        let (base64_data, thumb_width, thumb_height) = bgra_to_jpeg_thumbnail(
            &bgra_data,
            width,
            height,
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
        // Check permission first
        ensure_permission()?;

        // Validate region
        if width < 10 || height < 10 {
            return Err(CaptureError::InvalidRegion(format!(
                "Region must be at least 10x10 pixels (got {}x{})",
                width, height
            )));
        }

        // Parse monitor ID as display ID
        let display_id: CGDirectDisplayID = monitor_id.parse().map_err(|_| {
            CaptureError::InvalidParameters(format!("Invalid monitor ID: {}", monitor_id))
        })?;

        // Capture the display
        let image = capture_display(display_id)?;

        // Convert CGImage to BGRA data
        let (bgra_data, frame_width, frame_height) =
            cgimage_to_bgra(&image).map_err(CaptureError::PlatformError)?;

        // Get scale factor for coordinate conversion
        // Input coordinates are in logical pixels, captured image is in physical pixels
        let scale = get_monitor_scale_factor(monitor_id);

        // Convert logical coordinates to physical pixels for cropping
        let crop_x = (x as f64 * scale).round() as i32;
        let crop_y = (y as f64 * scale).round() as i32;
        let crop_width = (width as f64 * scale).round() as u32;
        let crop_height = (height as f64 * scale).round() as u32;

        // Crop the frame to region bounds
        let cropped = crop_frame(
            &bgra_data,
            frame_width,
            frame_height,
            crop_x,
            crop_y,
            crop_width,
            crop_height,
        );

        if cropped.is_empty() {
            return Err(CaptureError::PlatformError(
                "Crop resulted in empty frame".to_string(),
            ));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crop_frame_basic() {
        // Create a 4x4 test image (each pixel is BGRA = 4 bytes)
        let mut data = Vec::new();
        for i in 0u8..16 {
            data.extend_from_slice(&[i, i, i, 255]); // BGRA with pixel index as color
        }

        // Crop a 2x2 region starting at (1, 1)
        let cropped = crop_frame(&data, 4, 4, 1, 1, 2, 2);

        // Expected: pixels 5,6 and 9,10
        assert_eq!(cropped.len(), 2 * 2 * 4); // 2x2 pixels, 4 bytes each

        // Check pixel values
        assert_eq!(cropped[0..4], [5, 5, 5, 255]); // Pixel (1,1) = index 5
        assert_eq!(cropped[4..8], [6, 6, 6, 255]); // Pixel (2,1) = index 6
        assert_eq!(cropped[8..12], [9, 9, 9, 255]); // Pixel (1,2) = index 9
        assert_eq!(cropped[12..16], [10, 10, 10, 255]); // Pixel (2,2) = index 10
    }

    #[test]
    fn test_crop_frame_clamps_bounds() {
        // Create a 4x4 test image
        let data = vec![128u8; 4 * 4 * 4];

        // Crop with out-of-bounds region
        let cropped = crop_frame(&data, 4, 4, 3, 3, 10, 10);

        // Should clamp to 1x1
        assert_eq!(cropped.len(), 4);
    }

    #[test]
    fn test_crop_frame_empty_region() {
        let data = vec![128u8; 4 * 4 * 4];

        // Crop with zero-size region
        let cropped = crop_frame(&data, 4, 4, 0, 0, 0, 0);
        assert!(cropped.is_empty());
    }

    #[test]
    fn test_crop_frame_negative_coords() {
        let data = vec![128u8; 4 * 4 * 4];

        // Crop with negative coordinates (should clamp to 0)
        let cropped = crop_frame(&data, 4, 4, -1, -1, 2, 2);
        assert_eq!(cropped.len(), 2 * 2 * 4);
    }

    #[test]
    fn test_display_thumbnail_capture() {
        let capture = MacOSThumbnailCapture::new();
        let monitors = monitor_list::list_monitors();

        if monitors.is_empty() {
            println!("No monitors found, skipping test");
            return;
        }

        let result = capture.capture_display_thumbnail(&monitors[0].id);

        // Result depends on permission state
        match result {
            Ok(thumb) => {
                assert!(!thumb.data.is_empty(), "Thumbnail data should not be empty");
                assert!(thumb.width > 0, "Thumbnail width should be positive");
                assert!(thumb.height > 0, "Thumbnail height should be positive");
                assert!(
                    thumb.width <= THUMBNAIL_MAX_WIDTH,
                    "Thumbnail width should be <= max"
                );
                assert!(
                    thumb.height <= THUMBNAIL_MAX_HEIGHT,
                    "Thumbnail height should be <= max"
                );
                println!(
                    "Display thumbnail captured: {}x{}, {} bytes base64",
                    thumb.width,
                    thumb.height,
                    thumb.data.len()
                );
            }
            Err(CaptureError::PermissionDenied(_)) => {
                println!("Screen recording permission not granted, skipping test");
            }
            Err(e) => {
                println!(
                    "Display thumbnail capture failed (may be expected in CI): {}",
                    e
                );
            }
        }
    }

    #[test]
    fn test_region_preview_capture() {
        let capture = MacOSThumbnailCapture::new();
        let monitors = monitor_list::list_monitors();

        if monitors.is_empty() {
            println!("No monitors found, skipping test");
            return;
        }

        let monitor = &monitors[0];
        // Capture a 200x200 region from near the origin
        let x = 100;
        let y = 100;

        let result = capture.capture_region_preview(&monitor.id, x, y, 200, 200);

        match result {
            Ok(preview) => {
                assert!(!preview.data.is_empty(), "Preview data should not be empty");
                assert!(preview.width > 0, "Preview width should be positive");
                assert!(preview.height > 0, "Preview height should be positive");
                println!(
                    "Region preview captured: {}x{}, {} bytes base64",
                    preview.width,
                    preview.height,
                    preview.data.len()
                );
            }
            Err(CaptureError::PermissionDenied(_)) => {
                println!("Screen recording permission not granted, skipping test");
            }
            Err(e) => {
                println!(
                    "Region preview capture failed (may be expected in CI): {}",
                    e
                );
            }
        }
    }
}
