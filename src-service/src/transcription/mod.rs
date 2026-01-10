//! Voice transcription module for OmniRec.
//!
//! This module provides automatic transcription of audio captured during recording.
//! It uses whisper.cpp via FFI for speech-to-text conversion.
//!
//! ## Architecture
//!
//! The transcription pipeline consists of:
//!
//! 1. **Voice Detection** (`voice_detector.rs`) - Analyzes audio to detect speech segments
//!    using RMS amplitude, zero-crossing rate, and spectral centroid features.
//!
//! 2. **Segment Buffer** (`segment_buffer.rs`) - Ring buffer for continuous audio capture
//!    with segment extraction when speech ends or duration thresholds are reached.
//!
//! 3. **Transcription Queue** (`queue.rs`) - Bounded queue for async transcription processing
//!    with a worker thread that processes segments sequentially.
//!
//! 4. **Transcriber** (`transcriber.rs`) - Wrapper around whisper.cpp that handles model
//!    loading and audio transcription.
//!
//! 5. **Transcript Writer** (`transcript_writer.rs`) - Writes transcribed segments to
//!    a markdown file with timestamps.
//!
//! ## Configuration
//!
//! Unlike FlowSTT (optimized for short 2-4s dictation segments), OmniRec is tuned for
//! longer segments (up to 30s) where context and coherence matter more:
//!
//! - Segment duration threshold: 20s (vs FlowSTT's 4s)
//! - Grace period for word breaks: 2s (vs FlowSTT's 750ms)
//! - Absolute maximum segment: 30s
//! - Hold time after speech ends: 500ms (vs FlowSTT's 300ms)
//! - Word break detection: 150ms-500ms gaps (vs FlowSTT's 15ms-200ms)
//! - Ring buffer capacity: 35s (3,360,000 samples at 48kHz stereo)
//!
//! ## Model
//!
//! Uses the `ggml-medium.en` model (~1.5GB) for better accuracy on longer segments.
//! Model is downloaded on first use and cached in the platform-specific cache directory.

pub mod queue;
pub mod segment_buffer;
pub mod transcribe_state;
pub mod transcriber;
pub mod transcript_writer;
pub mod voice_detector;
pub mod whisper_ffi;

// Re-export main types for convenience
// These are public API for external use, even if not used internally
#[allow(unused_imports)]
pub use queue::{OnSegmentCallback, QueuedSegment, TranscriptionQueue};
#[allow(unused_imports)]
pub use segment_buffer::SegmentRingBuffer;
pub use transcribe_state::TranscribeState;
#[allow(unused_imports)]
pub use transcriber::{download_model, Transcriber};
#[allow(unused_imports)]
pub use transcript_writer::{transcript_filename_from_video, TranscriptWriter};
#[allow(unused_imports)]
pub use voice_detector::{SpeechStateChange, VoiceDetector, WordBreakEvent};
#[allow(unused_imports)]
pub use whisper_ffi::{
    get_default_model_path, get_system_info as get_whisper_system_info,
    init_library as init_whisper, Context as WhisperContext, WhisperFullParams,
    WhisperSamplingStrategy,
};
