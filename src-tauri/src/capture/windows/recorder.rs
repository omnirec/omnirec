//! Window recording using Windows.Graphics.Capture API via windows-capture crate.

use crate::capture::types::CapturedFrame;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use windows_capture::{
    capture::{Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
    window::Window,
};

/// Flags passed to the capture handler.
pub struct CaptureFlags {
    pub frame_tx: mpsc::Sender<CapturedFrame>,
    pub stop_flag: Arc<AtomicBool>,
}

/// Frame capture handler that sends frames through a channel.
struct CaptureHandler {
    frame_tx: mpsc::Sender<CapturedFrame>,
    stop_flag: Arc<AtomicBool>,
}

impl GraphicsCaptureApiHandler for CaptureHandler {
    type Flags = CaptureFlags;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            frame_tx: ctx.flags.frame_tx,
            stop_flag: ctx.flags.stop_flag,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        // Check if we should stop
        if self.stop_flag.load(Ordering::Relaxed) {
            capture_control.stop();
            return Ok(());
        }

        // Get frame buffer
        let mut buffer = frame.buffer()?;
        let width = buffer.width();
        let height = buffer.height();
        let raw_data = buffer.as_raw_buffer();

        // Calculate stride (bytes per row in the buffer) - may include padding for GPU alignment
        let buffer_stride = raw_data.len() / height as usize;
        let expected_stride = (width as usize) * 4; // BGRA = 4 bytes per pixel

        // Copy pixel data, handling stride padding if present
        let data = if buffer_stride == expected_stride {
            // No padding, copy directly
            raw_data.to_vec()
        } else {
            // Buffer has stride padding - extract only the actual pixel data row by row
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

        // Try to send frame, don't block if channel is full (drop frame instead)
        let _ = self.frame_tx.try_send(captured_frame);

        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        // Window was closed, signal stop
        self.stop_flag.store(true, Ordering::Relaxed);
        Ok(())
    }
}

/// Start capturing a window and return a receiver for frames.
///
/// Returns a tuple of (frame_receiver, stop_flag).
/// Set stop_flag to true to stop capture.
pub fn start_capture(
    window_handle: isize,
) -> Result<(mpsc::Receiver<CapturedFrame>, Arc<AtomicBool>), String> {
    // Find window by handle
    let window = Window::from_raw_hwnd(window_handle as *mut std::ffi::c_void);

    // Create channel for frames (bounded to prevent memory growth)
    let (frame_tx, frame_rx) = mpsc::channel::<CapturedFrame>(30); // ~1 second buffer at 30fps

    // Create stop flag
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();

    // Create flags for the handler
    let flags = CaptureFlags {
        frame_tx,
        stop_flag: stop_flag_clone,
    };

    // Configure capture settings with all required parameters
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

    // Start capture in a separate thread
    std::thread::spawn(move || {
        if let Err(e) = CaptureHandler::start(settings) {
            eprintln!("Capture error: {}", e);
        }
    });

    Ok((frame_rx, stop_flag))
}
