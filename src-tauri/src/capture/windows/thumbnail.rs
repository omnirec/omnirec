//! Windows thumbnail capture using Windows.Graphics.Capture API.
//!
//! This module captures single frames from monitors and windows for use as
//! thumbnails in the UI. It uses the `windows-capture` crate with a
//! "capture-and-stop" pattern to get a single frame efficiently.

use crate::capture::error::CaptureError;
use crate::capture::thumbnail::{
    bgra_to_jpeg_thumbnail, PREVIEW_MAX_HEIGHT, PREVIEW_MAX_WIDTH, THUMBNAIL_MAX_HEIGHT,
    THUMBNAIL_MAX_WIDTH,
};
use crate::capture::types::CapturedFrame;
use crate::capture::{ThumbnailCapture, ThumbnailResult};

use super::monitor_list;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use std::time::Duration;

use windows_capture::{
    capture::{Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
    window::Window,
};

/// Timeout for thumbnail capture operations.
const CAPTURE_TIMEOUT: Duration = Duration::from_millis(500);

/// Flags passed to the single-frame capture handler.
struct SingleFrameFlags {
    frame_tx: std_mpsc::SyncSender<CapturedFrame>,
    stop_flag: Arc<AtomicBool>,
}

/// Handler that captures a single frame and immediately stops.
struct SingleFrameHandler {
    frame_tx: std_mpsc::SyncSender<CapturedFrame>,
    stop_flag: Arc<AtomicBool>,
    captured: bool,
}

impl GraphicsCaptureApiHandler for SingleFrameHandler {
    type Flags = SingleFrameFlags;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            frame_tx: ctx.flags.frame_tx,
            stop_flag: ctx.flags.stop_flag,
            captured: false,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        // Only capture the first frame
        if self.captured || self.stop_flag.load(Ordering::Relaxed) {
            capture_control.stop();
            return Ok(());
        }

        // Get frame buffer
        let mut buffer = frame.buffer()?;
        let width = buffer.width();
        let height = buffer.height();
        let raw_data = buffer.as_raw_buffer();

        // Calculate stride (bytes per row in the buffer) - may include padding
        let buffer_stride = raw_data.len() / height as usize;
        let expected_stride = (width as usize) * 4; // BGRA = 4 bytes per pixel

        // Copy pixel data, handling stride padding if present
        let data = if buffer_stride == expected_stride {
            raw_data.to_vec()
        } else {
            let mut output = Vec::with_capacity(expected_stride * height as usize);
            for row in 0..height as usize {
                let src_start = row * buffer_stride;
                let src_end = src_start + expected_stride;
                if src_end <= raw_data.len() {
                    output.extend_from_slice(&raw_data[src_start..src_end]);
                }
            }
            output
        };

        let captured_frame = CapturedFrame {
            width,
            height,
            data,
        };

        // Send the frame (ignore errors - receiver may have timed out)
        let _ = self.frame_tx.send(captured_frame);
        self.captured = true;

        // Stop capture after first frame
        capture_control.stop();
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.stop_flag.store(true, Ordering::Relaxed);
        Ok(())
    }
}

/// Capture a single frame from a monitor.
fn capture_monitor_frame(monitor_id: &str) -> Result<CapturedFrame, CaptureError> {
    // Find monitor by device name
    let monitor = find_monitor_by_id(monitor_id)?;

    // Create sync channel for single frame
    let (frame_tx, frame_rx) = std_mpsc::sync_channel::<CapturedFrame>(1);
    let stop_flag = Arc::new(AtomicBool::new(false));

    let flags = SingleFrameFlags {
        frame_tx,
        stop_flag: stop_flag.clone(),
    };

    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::WithCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        flags,
    );

    // Start capture in background thread
    std::thread::spawn(move || {
        let _ = SingleFrameHandler::start(settings);
    });

    // Wait for frame with timeout
    frame_rx.recv_timeout(CAPTURE_TIMEOUT).map_err(|e| {
        stop_flag.store(true, Ordering::Relaxed);
        CaptureError::PlatformError(format!("Thumbnail capture timed out: {}", e))
    })
}

/// Capture a single frame from a window.
fn capture_window_frame(window_handle: isize) -> Result<CapturedFrame, CaptureError> {
    let window = Window::from_raw_hwnd(window_handle as *mut std::ffi::c_void);

    // Create sync channel for single frame
    let (frame_tx, frame_rx) = std_mpsc::sync_channel::<CapturedFrame>(1);
    let stop_flag = Arc::new(AtomicBool::new(false));

    let flags = SingleFrameFlags {
        frame_tx,
        stop_flag: stop_flag.clone(),
    };

    let settings = Settings::new(
        window,
        CursorCaptureSettings::WithCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        flags,
    );

    // Start capture in background thread
    std::thread::spawn(move || {
        let _ = SingleFrameHandler::start(settings);
    });

    // Wait for frame with timeout
    frame_rx.recv_timeout(CAPTURE_TIMEOUT).map_err(|e| {
        stop_flag.store(true, Ordering::Relaxed);
        CaptureError::PlatformError(format!("Window thumbnail capture timed out: {}", e))
    })
}

/// Find a monitor by its device ID.
fn find_monitor_by_id(monitor_id: &str) -> Result<Monitor, CaptureError> {
    let monitors = Monitor::enumerate()
        .map_err(|e| CaptureError::PlatformError(format!("Failed to enumerate monitors: {}", e)))?;

    for monitor in monitors {
        if let Ok(name) = monitor.device_name() {
            if name == monitor_id {
                return Ok(monitor);
            }
        }
    }

    Err(CaptureError::TargetNotFound(format!(
        "Monitor not found: {}",
        monitor_id
    )))
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

/// Windows thumbnail capture implementation.
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
    fn capture_window_thumbnail(
        &self,
        window_handle: isize,
    ) -> Result<ThumbnailResult, CaptureError> {
        // Capture single frame from window
        let frame = capture_window_frame(window_handle)?;

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

    fn capture_display_thumbnail(&self, monitor_id: &str) -> Result<ThumbnailResult, CaptureError> {
        // Capture single frame from monitor
        let frame = capture_monitor_frame(monitor_id)?;

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
        if width < 10 || height < 10 {
            return Err(CaptureError::InvalidRegion(format!(
                "Region must be at least 10x10 pixels (got {}x{})",
                width, height
            )));
        }

        // Capture single frame from monitor
        let frame = capture_monitor_frame(monitor_id)?;

        // Region coordinates are already in physical pixels (matching the frame buffer)
        // No conversion needed - crop directly
        let cropped = crop_frame(&frame.data, frame.width, frame.height, x, y, width, height);

        if cropped.is_empty() {
            return Err(CaptureError::PlatformError(
                "Crop resulted in empty frame".to_string(),
            ));
        }

        // Convert to preview (larger than thumbnail)
        let (base64_data, preview_width, preview_height) = bgra_to_jpeg_thumbnail(
            &cropped,
            width,
            height,
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
    fn test_display_thumbnail_capture() {
        let capture = WindowsThumbnailCapture::new();
        let monitors = monitor_list::list_monitors();

        if monitors.is_empty() {
            println!("No monitors found, skipping test");
            return;
        }

        let result = capture.capture_display_thumbnail(&monitors[0].id);

        // Should succeed on a real system
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
            Err(e) => {
                // May fail in CI/headless environments
                println!(
                    "Display thumbnail capture failed (may be expected in CI): {}",
                    e
                );
            }
        }
    }

    #[test]
    fn test_region_preview_capture() {
        let capture = WindowsThumbnailCapture::new();
        let monitors = monitor_list::list_monitors();

        if monitors.is_empty() {
            println!("No monitors found, skipping test");
            return;
        }

        let monitor = &monitors[0];
        // Capture a 200x200 region from the center
        let x = (monitor.width / 4) as i32;
        let y = (monitor.height / 4) as i32;

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
            Err(e) => {
                println!(
                    "Region preview capture failed (may be expected in CI): {}",
                    e
                );
            }
        }
    }
}
