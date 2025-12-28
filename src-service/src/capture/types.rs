//! Runtime types for capture operations (service-internal).
//!
//! These types are used for actual capture operations and are not
//! serializable for IPC. For IPC-compatible types, see omnirec-common.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::mpsc;

/// A captured frame with its dimensions and pixel data.
#[derive(Clone)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    /// BGRA pixel data
    pub data: Vec<u8>,
}

/// Handle to stop an ongoing capture.
pub type StopHandle = Arc<AtomicBool>;

/// Receiver for captured frames.
pub type FrameReceiver = mpsc::Receiver<CapturedFrame>;

/// A captured audio sample buffer.
#[derive(Clone)]
pub struct AudioSample {
    /// Audio data as interleaved f32 samples
    pub data: Vec<f32>,
    /// Sample rate in Hz (e.g., 48000)
    #[allow(dead_code)]
    pub sample_rate: u32,
    /// Number of audio channels (1 = mono, 2 = stereo)
    #[allow(dead_code)]
    pub channels: u32,
}

/// Receiver for captured audio samples.
pub type AudioReceiver = mpsc::Receiver<AudioSample>;
