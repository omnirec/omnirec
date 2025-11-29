//! Window recording using Windows.Graphics.Capture API via windows-capture crate.

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

/// A captured frame with its dimensions and pixel data.
#[derive(Clone)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    /// BGRA pixel data
    pub data: Vec<u8>,
}

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

        // Copy pixel data (BGRA format)
        let data = buffer.as_raw_buffer().to_vec();

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
