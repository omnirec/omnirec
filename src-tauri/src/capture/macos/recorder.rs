//! macOS screen capture using ScreenCaptureKit.
//!
//! Provides high-performance GPU-accelerated capture for displays and windows.

use crate::capture::types::{CapturedFrame, FrameReceiver, StopHandle};
use screencapturekit::{
    cm_sample_buffer::CMSampleBuffer,
    sc_content_filter::{InitParams, SCContentFilter},
    sc_error_handler::StreamErrorHandler,
    sc_output_handler::{SCStreamOutputType, StreamOutput},
    sc_shareable_content::SCShareableContent,
    sc_stream::SCStream,
    sc_stream_configuration::{PixelFormat, SCStreamConfiguration},
};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

// FFI bindings for CVPixelBuffer functions not exposed by the crate
#[link(name = "CoreVideo", kind = "framework")]
extern "C" {
    fn CVPixelBufferGetWidth(pixelBuffer: *const c_void) -> usize;
    fn CVPixelBufferGetHeight(pixelBuffer: *const c_void) -> usize;
    fn CVPixelBufferGetBytesPerRow(pixelBuffer: *const c_void) -> usize;
}

/// Frame rate for capture (frames per second).
const CAPTURE_FPS: f64 = 30.0;

/// Channel buffer size for frames.
const FRAME_BUFFER_SIZE: usize = 3;

/// Error handler for SCStream.
struct CaptureErrorHandler;

impl StreamErrorHandler for CaptureErrorHandler {
    fn on_error(&self) {
        eprintln!("[macOS] ScreenCaptureKit stream error");
    }
}

/// Frame output handler that converts CMSampleBuffer to CapturedFrame.
struct FrameOutputHandler {
    tx: mpsc::Sender<CapturedFrame>,
    stop_flag: Arc<AtomicBool>,
    width: u32,
    height: u32,
}

impl StreamOutput for FrameOutputHandler {
    fn did_output_sample_buffer(&self, sample: CMSampleBuffer, of_type: SCStreamOutputType) {
        // Only handle screen output
        if !matches!(of_type, SCStreamOutputType::Screen) {
            return;
        }

        // Check if we should stop
        if self.stop_flag.load(Ordering::Relaxed) {
            return;
        }

        // Get pixel buffer - skip frames without valid pixel data
        let pixel_buffer = match &sample.pixel_buffer {
            Some(pb) => pb,
            None => return,
        };

        // Lock the buffer for reading
        if !pixel_buffer.lock() {
            eprintln!("[macOS] Failed to lock pixel buffer");
            return;
        }

        // Get pixel data
        // ScreenCaptureKit uses BGRA format when configured with ARGB8888
        let base_address = pixel_buffer.get_base_adress();
        if base_address.is_null() {
            pixel_buffer.unlock();
            return;
        }

        // Get the CVPixelBuffer pointer from the image buffer ref
        // CVImageBufferRef and CVPixelBufferRef are toll-free bridged (same type)
        // The ShareId<CVImageBufferRef> derefs to CVImageBufferRef, and we need the raw pointer
        let cv_buffer_ptr: *const c_void = sample.image_buf_ref.as_ref()
            .map(|img_buf| {
                // ShareId<T> implements Deref<Target = T>, so &**img_buf gives us &T
                // Then we take a pointer to it. Since T is the opaque CVImageBufferRef,
                // the pointer is the CVImageBufferRef pointer (CVPixelBufferRef).
                let inner_ref = &**img_buf;
                inner_ref as *const _ as *const c_void
            })
            .unwrap_or(std::ptr::null());
        
        let (actual_width, actual_height, bytes_per_row) = if !cv_buffer_ptr.is_null() {
            unsafe {
                (
                    CVPixelBufferGetWidth(cv_buffer_ptr) as u32,
                    CVPixelBufferGetHeight(cv_buffer_ptr) as u32,
                    CVPixelBufferGetBytesPerRow(cv_buffer_ptr),
                )
            }
        } else {
            (self.width, self.height, (self.width * 4) as usize)
        };
        
        let width = if actual_width > 0 { actual_width } else { self.width };
        let height = if actual_height > 0 { actual_height } else { self.height };
        let expected_bytes_per_row = (width * 4) as usize;
        
        // Log first frame info for debugging
        static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !LOGGED.swap(true, Ordering::Relaxed) {
            eprintln!("[macOS] Frame info: {}x{}, bytes_per_row={}, expected={}", 
                width, height, bytes_per_row, expected_bytes_per_row);
        }

        // If stride matches expected, we can copy directly
        let data = if bytes_per_row == expected_bytes_per_row || bytes_per_row == 0 {
            let data_size = (width * height * 4) as usize;
            let src = unsafe { std::slice::from_raw_parts(base_address as *const u8, data_size) };
            src.to_vec()
        } else {
            // Stride doesn't match - need to copy row by row, removing padding
            let mut data = Vec::with_capacity((width * height * 4) as usize);
            let src_ptr = base_address as *const u8;
            
            for row in 0..height {
                let row_start = (row as usize) * bytes_per_row;
                let row_data = unsafe {
                    std::slice::from_raw_parts(src_ptr.add(row_start), expected_bytes_per_row)
                };
                data.extend_from_slice(row_data);
            }
            data
        };

        let frame = CapturedFrame {
            width,
            height,
            data,
        };

        // Unlock the buffer
        pixel_buffer.unlock();

        // Send frame (non-blocking)
        let _ = self.tx.try_send(frame);
    }
}

/// Start capturing a display.
///
/// Returns a frame receiver and stop handle.
pub fn start_display_capture(
    display_id: u32,
    width: u32,
    height: u32,
) -> Result<(FrameReceiver, StopHandle), String> {
    eprintln!(
        "[macOS] Starting display capture for display {} ({}x{})",
        display_id, width, height
    );

    // Get shareable content (this requires screen recording permission)
    let content = SCShareableContent::try_current()
        .map_err(|e| format!("Failed to get shareable content: {}", e))?;

    // Find the display and take ownership
    let display_index = content
        .displays
        .iter()
        .position(|d| d.display_id == display_id)
        .ok_or_else(|| format!("Display {} not found", display_id))?;

    // We need to take the display out of the content to give ownership to SCContentFilter
    // Since we can't move out of SCShareableContent, we need to work around this
    // by creating the filter with the display reference
    let mut content = content;
    let display = content.displays.swap_remove(display_index);

    let filter = SCContentFilter::new(InitParams::Display(display));

    // Configure stream
    let config = SCStreamConfiguration {
        width,
        height,
        shows_cursor: true,
        pixel_format: PixelFormat::ARGB8888, // BGRA in memory
        minimum_frame_interval: screencapturekit::sc_types::base::CMTime {
            value: 1,
            timescale: CAPTURE_FPS as i32,
            flags: 1,
            epoch: 0,
        },
        queue_depth: FRAME_BUFFER_SIZE as u32,
        ..Default::default()
    };

    // Create channel for frames
    let (tx, rx) = mpsc::channel(FRAME_BUFFER_SIZE);

    // Create stop flag
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();

    // Create stream
    let mut stream = SCStream::new(filter, config, CaptureErrorHandler);

    // Add output handler
    let handler = FrameOutputHandler {
        tx,
        stop_flag: stop_flag.clone(),
        width,
        height,
    };
    stream.add_output(handler, SCStreamOutputType::Screen);

    // Start capture
    stream
        .start_capture()
        .map_err(|e| format!("Failed to start capture: {}", e))?;

    eprintln!("[macOS] Display capture started");

    // Keep stream alive by storing it
    // The stream will be stopped when stop_flag is set
    std::thread::spawn(move || {
        while !stop_flag.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        eprintln!("[macOS] Stopping display capture");
        let _ = stream.stop_capture();
    });

    Ok((rx, stop_flag_clone))
}

/// Start capturing a window.
///
/// Returns a frame receiver and stop handle.
pub fn start_window_capture(window_id: u32) -> Result<(FrameReceiver, StopHandle), String> {
    eprintln!("[macOS] Starting window capture for window {}", window_id);

    // Get shareable content
    let content = SCShareableContent::try_current()
        .map_err(|e| format!("Failed to get shareable content: {}", e))?;

    // Find the window
    let window_index = content
        .windows
        .iter()
        .position(|w| w.window_id == window_id)
        .ok_or_else(|| format!("Window {} not found", window_id))?;

    // Take ownership of the window
    let mut content = content;
    let window = content.windows.swap_remove(window_index);

    // Get window dimensions (SCWindow has width/height fields directly)
    let width = window.width;
    let height = window.height;

    if width == 0 || height == 0 {
        return Err("Window has zero dimensions".to_string());
    }

    eprintln!("[macOS] Window size: {}x{}", width, height);

    // Create content filter for the window (desktop-independent)
    let filter = SCContentFilter::new(InitParams::DesktopIndependentWindow(window));

    // Configure stream
    let config = SCStreamConfiguration {
        width,
        height,
        shows_cursor: true,
        pixel_format: PixelFormat::ARGB8888,
        minimum_frame_interval: screencapturekit::sc_types::base::CMTime {
            value: 1,
            timescale: CAPTURE_FPS as i32,
            flags: 1,
            epoch: 0,
        },
        queue_depth: FRAME_BUFFER_SIZE as u32,
        ..Default::default()
    };

    // Create channel for frames
    let (tx, rx) = mpsc::channel(FRAME_BUFFER_SIZE);

    // Create stop flag
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();

    // Create stream
    let mut stream = SCStream::new(filter, config, CaptureErrorHandler);

    // Add output handler
    let handler = FrameOutputHandler {
        tx,
        stop_flag: stop_flag.clone(),
        width,
        height,
    };
    stream.add_output(handler, SCStreamOutputType::Screen);

    // Start capture
    stream
        .start_capture()
        .map_err(|e| format!("Failed to start capture: {}", e))?;

    eprintln!("[macOS] Window capture started");

    // Keep stream alive
    std::thread::spawn(move || {
        while !stop_flag.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        eprintln!("[macOS] Stopping window capture");
        let _ = stream.stop_capture();
    });

    Ok((rx, stop_flag_clone))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_fps() {
        assert_eq!(CAPTURE_FPS, 30.0);
    }
}
