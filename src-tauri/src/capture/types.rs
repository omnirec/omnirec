//! Runtime types for capture operations (service-internal).
//!
//! These types are used for actual capture operations and are not
//! serializable for IPC. For IPC-compatible types, see omnirec-common.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::mpsc;

/// A captured frame with its dimensions and pixel data.
#[derive(Clone)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    /// BGRA pixel data
    pub data: Vec<u8>,
    /// Wall-clock time when this frame was captured by the OS.
    ///
    /// This is stamped with `SystemTime::now()` in the platform capture
    /// callback (e.g. `on_frame_arrived`), as close to the actual capture
    /// moment as possible.  The encoder uses this to align audio and video
    /// on a common absolute timeline.
    pub captured_at: SystemTime,
}

/// Handle to stop an ongoing capture.
pub type StopHandle = Arc<AtomicBool>;

/// Receiver for captured frames.
pub type FrameReceiver = mpsc::Receiver<CapturedFrame>;
