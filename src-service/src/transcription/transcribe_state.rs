//! Transcription state management for OmniRec.
//!
//! This module manages the transcription pipeline during recording:
//! - Receives audio samples from the capture pipeline
//! - Resamples from capture format (48kHz stereo) to whisper format (16kHz mono)
//! - Detects speech segments using the voice detector
//! - Queues segments for transcription
//!
//! ## Audio Flow
//!
//! ```text
//! Audio Capture (48kHz stereo)
//!        │
//!        ▼
//! TranscribeState.process_samples()
//!        │
//!        ├──► Resample to 16kHz mono
//!        │
//!        ▼
//! SegmentRingBuffer (stores 16kHz mono)
//!        │
//!        ▼
//! VoiceDetector (speech segmentation)
//!        │
//!        ▼
//! TranscriptionQueue (async processing)
//!        │
//!        ▼
//! Transcriber (whisper.cpp)
//!        │
//!        ▼
//! TranscriptWriter (markdown output)
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use super::queue::{OnSegmentCallback, QueuedSegment, TranscriptionQueue};
use super::segment_buffer::SegmentRingBuffer;
use super::transcript_writer::transcript_filename_from_video;
use super::voice_detector::{SpeechStateChange, VoiceDetector};

/// Whisper expects 16kHz mono audio
const WHISPER_SAMPLE_RATE: u32 = 16000;

/// Maximum segment duration before forced submission (30 seconds)
const MAX_SEGMENT_DURATION_SECS: f64 = 30.0;

/// Segment duration threshold before seeking word break (20 seconds)
const SEGMENT_THRESHOLD_SECS: f64 = 20.0;

/// Grace period after threshold before forced submission (2 seconds)
const WORD_BREAK_GRACE_SECS: f64 = 2.0;

/// Minimum segment duration for transcription (500ms)
const MIN_SEGMENT_DURATION_SECS: f64 = 0.5;

/// Minimum RMS amplitude for non-silent audio
const MIN_AUDIO_RMS: f32 = 0.01;

/// Transcription state for a recording session.
///
/// Manages the transcription pipeline during recording, coordinating
/// voice detection, segment buffering, and transcription queue.
pub struct TranscribeState {
    /// Whether transcription is active
    is_active: bool,
    /// Ring buffer for audio segments (16kHz mono)
    ring_buffer: SegmentRingBuffer,
    /// Voice activity detector
    voice_detector: VoiceDetector,
    /// Transcription queue (shared with worker thread)
    transcription_queue: Arc<TranscriptionQueue>,
    /// Whether we're currently inside a speech segment
    in_speech: bool,
    /// Ring buffer index where current speech segment started
    segment_start_idx: usize,
    /// Number of samples in the current segment
    segment_sample_count: u64,
    /// Lookback samples at start of current segment
    lookback_sample_count: usize,
    /// Whether we're seeking a word break (duration threshold exceeded)
    seeking_word_break: bool,
    /// Sample count when we started seeking word break
    word_break_seek_start_samples: u64,
    /// Recording start time (for timestamps)
    recording_start: Option<Instant>,
    /// Output path for video file (used to derive transcript path)
    output_path: Option<PathBuf>,
    /// Resampler state: accumulated samples for downsampling
    resample_buffer: Vec<f32>,
    /// Input sample rate (typically 48000)
    input_sample_rate: u32,
    /// Input channels (typically 2)
    input_channels: u16,
}

impl TranscribeState {
    /// Create a new transcription state.
    pub fn new() -> Self {
        Self {
            is_active: false,
            ring_buffer: SegmentRingBuffer::with_default_capacity(),
            voice_detector: VoiceDetector::new(WHISPER_SAMPLE_RATE),
            transcription_queue: Arc::new(TranscriptionQueue::new()),
            in_speech: false,
            segment_start_idx: 0,
            segment_sample_count: 0,
            lookback_sample_count: 0,
            seeking_word_break: false,
            word_break_seek_start_samples: 0,
            recording_start: None,
            output_path: None,
            resample_buffer: Vec::new(),
            input_sample_rate: 48000,
            input_channels: 2,
        }
    }

    /// Check if transcription is active.
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Get the transcription queue.
    pub fn queue(&self) -> Arc<TranscriptionQueue> {
        Arc::clone(&self.transcription_queue)
    }

    /// Start transcription for a recording.
    ///
    /// # Arguments
    /// * `output_path` - Path to the video output file (transcript will be derived from this)
    /// * `input_sample_rate` - Sample rate of input audio (typically 48000)
    /// * `input_channels` - Number of channels in input audio (typically 2)
    /// * `model_path` - Optional custom model path (uses default if None)
    pub fn start_with_model(
        &mut self,
        output_path: PathBuf,
        input_sample_rate: u32,
        input_channels: u16,
        model_path: Option<PathBuf>,
    ) -> Result<(), String> {
        self.start_with_model_and_callback(
            output_path,
            input_sample_rate,
            input_channels,
            model_path,
            None,
        )
    }

    /// Start transcription for a recording with an optional callback for segment events.
    ///
    /// # Arguments
    /// * `output_path` - Path to the video output file (transcript will be derived from this)
    /// * `input_sample_rate` - Sample rate of input audio (typically 48000)
    /// * `input_channels` - Number of channels in input audio (typically 2)
    /// * `model_path` - Optional custom model path (uses default if None)
    /// * `on_segment` - Optional callback invoked when a segment is transcribed
    pub fn start_with_model_and_callback(
        &mut self,
        output_path: PathBuf,
        input_sample_rate: u32,
        input_channels: u16,
        model_path: Option<PathBuf>,
        on_segment: Option<Arc<OnSegmentCallback>>,
    ) -> Result<(), String> {
        if self.is_active {
            return Err("Transcription already active".to_string());
        }

        // Derive transcript path from video path
        let transcript_path = transcript_filename_from_video(&output_path);

        // Reset state
        self.ring_buffer.clear();
        self.voice_detector.reset();
        self.in_speech = false;
        self.segment_start_idx = 0;
        self.segment_sample_count = 0;
        self.lookback_sample_count = 0;
        self.seeking_word_break = false;
        self.word_break_seek_start_samples = 0;
        self.resample_buffer.clear();
        self.input_sample_rate = input_sample_rate;
        self.input_channels = input_channels;
        self.recording_start = Some(Instant::now());
        self.output_path = Some(output_path);

        eprintln!(
            "[TranscribeState] Started - transcript: {:?}, sample_rate: {}, channels: {}, model: {:?}",
            transcript_path, input_sample_rate, input_channels, model_path
        );

        // Start the transcription worker with optional callback
        self.transcription_queue.start_worker_with_callback(
            transcript_path,
            model_path,
            on_segment,
        );

        self.is_active = true;

        Ok(())
    }

    /// Stop transcription.
    ///
    /// Finalizes any in-progress segment and stops the worker thread.
    pub fn stop(&mut self) {
        if !self.is_active {
            return;
        }

        // Finalize any in-progress segment
        if self.in_speech {
            self.finalize_current_segment();
        }

        // Stop the worker (will drain queue)
        self.transcription_queue.stop_worker();

        self.is_active = false;
        self.recording_start = None;
        self.output_path = None;

        tracing::info!(
            "Transcription stopped ({} segments processed)",
            self.transcription_queue.segments_processed()
        );
    }

    /// Process audio samples from the capture pipeline.
    ///
    /// Samples are expected in the capture format (typically 48kHz stereo f32).
    /// They will be resampled to 16kHz mono for whisper.
    pub fn process_samples(&mut self, samples: &[f32]) {
        if !self.is_active {
            return;
        }

        // Resample to 16kHz mono
        let resampled = self.resample_to_whisper_format(samples);
        if resampled.is_empty() {
            return;
        }

        // Log first sample processing
        static FIRST_SAMPLE_LOGGED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if !FIRST_SAMPLE_LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            eprintln!(
                "[TranscribeState] First samples received: {} input -> {} resampled",
                samples.len(),
                resampled.len()
            );
        }

        // Write to ring buffer
        self.ring_buffer.write(&resampled);

        // Process through voice detector
        // Use chunks that match the detector's expectations (~10ms at 16kHz = 160 samples)
        const CHUNK_SIZE: usize = 160;
        for chunk in resampled.chunks(CHUNK_SIZE) {
            if chunk.len() < CHUNK_SIZE / 2 {
                continue; // Skip very small chunks
            }

            self.voice_detector.process(chunk);

            // Handle speech state changes
            match self.voice_detector.take_state_change() {
                SpeechStateChange::Started { lookback_samples } => {
                    self.on_speech_started(lookback_samples);
                }
                SpeechStateChange::Ended { duration_ms: _ } => {
                    self.on_speech_ended();
                }
                SpeechStateChange::None => {}
            }

            // Handle word breaks if seeking
            if self.seeking_word_break {
                if let Some(word_break) = self.voice_detector.take_word_break_event() {
                    self.on_word_break(word_break.offset_ms, word_break.gap_duration_ms);
                }
            }

            // Update segment tracking
            if self.in_speech {
                self.segment_sample_count += chunk.len() as u64;
                self.check_segment_duration();
            }
        }
    }

    /// Resample audio from capture format to whisper format (16kHz mono).
    fn resample_to_whisper_format(&mut self, samples: &[f32]) -> Vec<f32> {
        // Convert stereo to mono first (if stereo)
        let mono: Vec<f32> = if self.input_channels >= 2 {
            samples
                .chunks(self.input_channels as usize)
                .map(|frame| {
                    // Average all channels
                    let sum: f32 = frame.iter().sum();
                    sum / self.input_channels as f32
                })
                .collect()
        } else {
            samples.to_vec()
        };

        // Add to resample buffer
        self.resample_buffer.extend_from_slice(&mono);

        // Calculate resampling ratio
        // 48000 -> 16000 = 3:1 ratio
        let ratio = self.input_sample_rate as f32 / WHISPER_SAMPLE_RATE as f32;

        // Simple decimation with averaging (adequate for voice)
        let ratio_int = ratio.round() as usize;
        if ratio_int <= 1 {
            // No resampling needed
            return std::mem::take(&mut self.resample_buffer);
        }

        // Output every ratio_int samples, averaging the window
        let output_len = self.resample_buffer.len() / ratio_int;
        if output_len == 0 {
            return Vec::new();
        }

        let mut output = Vec::with_capacity(output_len);
        for i in 0..output_len {
            let start = i * ratio_int;
            let end = (start + ratio_int).min(self.resample_buffer.len());
            let sum: f32 = self.resample_buffer[start..end].iter().sum();
            output.push(sum / (end - start) as f32);
        }

        // Keep remainder for next call
        let consumed = output_len * ratio_int;
        self.resample_buffer.drain(0..consumed);

        output
    }

    /// Handle speech started event.
    fn on_speech_started(&mut self, lookback_samples: usize) {
        if self.in_speech {
            return; // Already in speech
        }

        self.in_speech = true;
        self.segment_start_idx = self.ring_buffer.index_from_lookback(lookback_samples);
        self.segment_sample_count = 0;
        self.lookback_sample_count = lookback_samples;
        self.seeking_word_break = false;

        eprintln!(
            "[TranscribeState] Speech STARTED (lookback: {} samples, idx: {})",
            lookback_samples, self.segment_start_idx
        );
    }

    /// Handle speech ended event.
    fn on_speech_ended(&mut self) {
        if !self.in_speech {
            return;
        }

        let duration_secs = self.segment_sample_count as f64 / WHISPER_SAMPLE_RATE as f64;
        eprintln!("[TranscribeState] Speech ENDED after {:.2}s", duration_secs);

        self.finalize_current_segment();
    }

    /// Handle word break event during seeking.
    fn on_word_break(&mut self, offset_ms: u32, _gap_duration_ms: u32) {
        if !self.in_speech || !self.seeking_word_break {
            return;
        }

        // Calculate extraction point (at the word break)
        let offset_samples = (offset_ms as u64 * WHISPER_SAMPLE_RATE as u64 / 1000) as usize;
        let extraction_length = self.lookback_sample_count + offset_samples;

        // Extract segment up to word break
        let end_idx = (self.segment_start_idx + extraction_length) % self.ring_buffer.capacity();
        let segment = self
            .ring_buffer
            .extract_segment_to(self.segment_start_idx, end_idx);

        if self.validate_and_queue_segment(segment) {
            // Update state for continuation
            self.segment_start_idx = end_idx;
            self.lookback_sample_count = 0;
            self.segment_sample_count = self
                .segment_sample_count
                .saturating_sub(offset_samples as u64);
            self.seeking_word_break = false;
        }
    }

    /// Check segment duration and handle thresholds.
    fn check_segment_duration(&mut self) {
        let duration_secs = self.segment_sample_count as f64 / WHISPER_SAMPLE_RATE as f64;

        // Check for absolute maximum
        if duration_secs >= MAX_SEGMENT_DURATION_SECS {
            tracing::debug!("Max segment duration reached, force extracting");
            self.finalize_current_segment();
            return;
        }

        // Check if we should start seeking word break
        if !self.seeking_word_break && duration_secs >= SEGMENT_THRESHOLD_SECS {
            self.seeking_word_break = true;
            self.word_break_seek_start_samples = self.segment_sample_count;
            tracing::debug!(
                "Segment threshold reached ({:.1}s), seeking word break",
                duration_secs
            );
        }

        // Check grace period expiration
        if self.seeking_word_break {
            let samples_since_seek = self.segment_sample_count - self.word_break_seek_start_samples;
            let grace_secs = samples_since_seek as f64 / WHISPER_SAMPLE_RATE as f64;

            if grace_secs >= WORD_BREAK_GRACE_SECS {
                tracing::debug!("Grace period expired, force extracting");
                self.finalize_current_segment();
            }
        }
    }

    /// Finalize the current segment and queue for transcription.
    fn finalize_current_segment(&mut self) {
        if !self.in_speech {
            return;
        }

        // Extract segment from ring buffer
        let segment = self.ring_buffer.extract_segment(self.segment_start_idx);

        self.validate_and_queue_segment(segment);

        // Reset state
        self.in_speech = false;
        self.segment_sample_count = 0;
        self.lookback_sample_count = 0;
        self.seeking_word_break = false;
    }

    /// Validate and queue a segment for transcription.
    ///
    /// Returns true if the segment was queued.
    fn validate_and_queue_segment(&mut self, segment: Vec<f32>) -> bool {
        if segment.is_empty() {
            return false;
        }

        // Check minimum duration
        let duration_secs = segment.len() as f64 / WHISPER_SAMPLE_RATE as f64;
        if duration_secs < MIN_SEGMENT_DURATION_SECS {
            tracing::debug!(
                "Segment too short ({:.2}s < {:.2}s), skipping",
                duration_secs,
                MIN_SEGMENT_DURATION_SECS
            );
            return false;
        }

        // Check RMS amplitude
        let sum_squares: f32 = segment.iter().map(|s| s * s).sum();
        let rms = (sum_squares / segment.len() as f32).sqrt();
        if rms < MIN_AUDIO_RMS {
            tracing::debug!(
                "Segment too quiet (RMS {:.4} < {:.4}), skipping",
                rms,
                MIN_AUDIO_RMS
            );
            return false;
        }

        // Calculate timestamp
        let timestamp_secs = self
            .recording_start
            .map(|start| start.elapsed().as_secs_f64())
            .unwrap_or(0.0);

        // Queue segment
        let queued = QueuedSegment {
            samples: segment,
            timestamp_secs,
        };

        if self.transcription_queue.enqueue(queued) {
            eprintln!(
                "[TranscribeState] QUEUED segment: {:.2}s at timestamp {:.1}s (queue depth: {}, RMS: {:.4})",
                duration_secs,
                timestamp_secs,
                self.transcription_queue.queue_depth(),
                rms
            );
            true
        } else {
            eprintln!("[TranscribeState] Queue full, segment dropped");
            false
        }
    }
}

impl Default for TranscribeState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcribe_state_creation() {
        let state = TranscribeState::new();
        assert!(!state.is_active());
    }

    #[test]
    fn test_resample_stereo_to_mono() {
        let mut state = TranscribeState::new();
        state.input_sample_rate = 48000;
        state.input_channels = 2;

        // Stereo samples: L=1.0, R=0.0, L=0.0, R=1.0
        let stereo = vec![1.0, 0.0, 0.0, 1.0];
        let resampled = state.resample_to_whisper_format(&stereo);

        // At 48->16kHz with 4 samples input, we don't get any output yet
        // (need 3 input samples for 1 output sample, but we have 2 mono samples)
        // This tests the buffering behavior
        assert!(resampled.is_empty() || resampled.len() <= 1);
    }
}
