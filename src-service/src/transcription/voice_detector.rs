//! Voice activity detection for transcription segmentation.
//!
//! This module provides speech detection using multi-feature analysis:
//! - RMS amplitude for basic energy detection
//! - Zero-Crossing Rate (ZCR) to distinguish voiced speech from transients
//! - Spectral centroid approximation to identify speech-band frequency content
//!
//! Implements dual-mode detection:
//! - **Voiced mode**: For normal speech (lower ZCR, speech-band centroid)
//! - **Whisper mode**: For soft/breathy speech (higher ZCR, broader centroid range)
//!
//! Ported from FlowSTT's `processor.rs` and tuned for OmniRec's longer segments.

/// Speech state change detected by the voice detector
#[derive(Clone, Debug)]
pub enum SpeechStateChange {
    /// No change in speech state
    None,
    /// Speech started with lookback sample count
    Started { lookback_samples: usize },
    /// Speech ended with duration in milliseconds
    Ended {
        #[allow(dead_code)]
        duration_ms: u64,
    },
}

/// Word break event detected during speech
#[derive(Clone, Debug)]
pub struct WordBreakEvent {
    /// Offset from speech start in milliseconds
    pub offset_ms: u32,
    /// Duration of the gap in milliseconds
    pub gap_duration_ms: u32,
}

/// Configuration for a speech detection mode (voiced or whisper)
#[derive(Clone)]
struct SpeechModeConfig {
    /// Minimum amplitude threshold in dB
    threshold_db: f32,
    /// ZCR range (min, max) - normalized as crossings per sample
    zcr_range: (f32, f32),
    /// Spectral centroid range in Hz (min, max)
    centroid_range: (f32, f32),
    /// Onset time in samples before confirming speech
    onset_samples: u32,
}

/// Voice activity detector that identifies speech segments in audio.
///
/// Uses multi-feature analysis for robust speech detection:
/// - RMS amplitude for basic energy detection
/// - Zero-Crossing Rate (ZCR) to distinguish voiced speech from transients
/// - Spectral centroid approximation to identify speech-band frequency content
///
/// Includes lookback functionality to capture the true start of speech by maintaining
/// a ring buffer of recent audio samples and analyzing them retroactively.
///
/// ## OmniRec-specific tuning
///
/// Compared to FlowSTT (optimized for short dictation), OmniRec uses:
/// - Hold time: 500ms (vs 300ms) - longer pause tolerance for continuous recording
/// - Word break min gap: 150ms (vs 15ms) - only break on clear pauses
/// - Word break max gap: 500ms (vs 200ms) - allow longer natural pauses
pub struct VoiceDetector {
    /// Sample rate for time/frequency calculations
    sample_rate: u32,
    /// Voiced speech detection configuration
    voiced_config: SpeechModeConfig,
    /// Whisper speech detection configuration
    whisper_config: SpeechModeConfig,
    /// Transient rejection: ZCR threshold (reject if above)
    transient_zcr_threshold: f32,
    /// Transient rejection: centroid threshold in Hz (reject if above, combined with ZCR)
    transient_centroid_threshold: f32,
    /// Hold time in samples before emitting speech-ended event
    hold_samples: u32,
    /// Current speech state (true = speaking, false = silent)
    is_speaking: bool,
    /// Whether we're in "pending voiced" state
    is_pending_voiced: bool,
    /// Whether we're in "pending whisper" state
    is_pending_whisper: bool,
    /// Counter for voiced onset time
    voiced_onset_count: u32,
    /// Counter for whisper onset time
    whisper_onset_count: u32,
    /// Counter for hold time during silence
    silence_sample_count: u32,
    /// Counter for speech duration (from confirmed start)
    speech_sample_count: u64,
    /// Grace samples allowed during onset (brief dips don't reset counters)
    onset_grace_samples: u32,
    /// Current grace counter for voiced onset
    voiced_grace_count: u32,
    /// Current grace counter for whisper onset
    whisper_grace_count: u32,
    /// Whether we've initialized (first sample processed)
    initialized: bool,
    /// Last computed amplitude in dB
    last_amplitude_db: f32,
    /// Last computed ZCR
    last_zcr: f32,
    /// Last computed spectral centroid in Hz
    last_centroid_hz: f32,
    /// Whether last frame was classified as transient
    last_is_transient: bool,

    // Lookback ring buffer fields
    /// Ring buffer for recent audio samples (for lookback analysis)
    lookback_buffer: Vec<f32>,
    /// Current write position in the ring buffer
    lookback_write_index: usize,
    /// Capacity of the lookback buffer in samples
    lookback_capacity: usize,
    /// Whether the lookback buffer has been filled at least once
    lookback_filled: bool,
    /// Lookback threshold in dB (more sensitive than detection threshold)
    lookback_threshold_db: f32,
    /// Last lookback offset in milliseconds (set when speech confirmed)
    last_lookback_offset_ms: Option<u32>,
    /// Last state change detected during process()
    last_state_change: SpeechStateChange,

    // Word break detection fields
    /// Word break threshold ratio (amplitude must drop below this fraction of recent average)
    word_break_threshold_ratio: f32,
    /// Minimum gap duration in samples for word break (150ms for OmniRec)
    min_word_break_samples: u32,
    /// Maximum gap duration in samples for word break (500ms for OmniRec)
    max_word_break_samples: u32,
    /// Window size in samples for tracking recent speech amplitude (100ms)
    recent_speech_window_samples: u32,
    /// Running sum of recent speech amplitude (linear, not dB)
    recent_speech_amplitude_sum: f32,
    /// Count of samples in recent speech amplitude window
    recent_speech_amplitude_count: u32,
    /// Whether we're currently in a word break gap
    in_word_break: bool,
    /// Sample count of current word break gap
    word_break_sample_count: u32,
    /// Sample count at start of current word break (for offset calculation)
    word_break_start_speech_samples: u64,
    /// Whether last frame was a word break
    last_is_word_break: bool,
    /// Last word break event detected
    last_word_break_event: Option<WordBreakEvent>,
    /// Total samples processed (for periodic debug logging)
    total_samples_processed: u64,
    /// Last debug log time
    last_debug_log_samples: u64,
}

impl VoiceDetector {
    /// Create a new voice detector with specified sample rate.
    /// Uses default configuration optimized for OmniRec's longer segments.
    pub fn new(sample_rate: u32) -> Self {
        Self::with_defaults(sample_rate)
    }

    /// Create a voice detector with default configuration.
    ///
    /// Default parameters (tuned for OmniRec's longer segments):
    /// - Voiced mode: -42dB threshold, ZCR 0.01-0.30, centroid 200-5500Hz, 80ms onset
    /// - Whisper mode: -52dB threshold, ZCR 0.08-0.45, centroid 300-7000Hz, 120ms onset
    /// - Transient rejection: ZCR > 0.45 AND centroid > 6500Hz
    /// - Hold time: 500ms (longer than FlowSTT's 300ms)
    /// - Onset grace period: 30ms (brief dips in features don't reset onset counters)
    /// - Lookback buffer: 200ms (covers max onset time + margin)
    /// - Lookback threshold: -55dB (more sensitive to catch speech starts)
    /// - Word break: 150ms-500ms gaps (vs FlowSTT's 15ms-200ms)
    pub fn with_defaults(sample_rate: u32) -> Self {
        // OmniRec uses longer hold time (500ms vs FlowSTT's 300ms)
        let hold_samples = (sample_rate as u64 * 500 / 1000) as u32;
        // 200ms lookback buffer
        let lookback_capacity = (sample_rate as u64 * 200 / 1000) as usize;

        Self {
            sample_rate,
            voiced_config: SpeechModeConfig {
                threshold_db: -42.0,
                zcr_range: (0.01, 0.30),
                centroid_range: (200.0, 5500.0),
                onset_samples: (sample_rate as u64 * 80 / 1000) as u32,
            },
            whisper_config: SpeechModeConfig {
                threshold_db: -52.0,
                zcr_range: (0.08, 0.45),
                centroid_range: (300.0, 7000.0),
                onset_samples: (sample_rate as u64 * 120 / 1000) as u32,
            },
            transient_zcr_threshold: 0.45,
            transient_centroid_threshold: 6500.0,
            hold_samples,
            is_speaking: false,
            is_pending_voiced: false,
            is_pending_whisper: false,
            voiced_onset_count: 0,
            whisper_onset_count: 0,
            silence_sample_count: 0,
            speech_sample_count: 0,
            onset_grace_samples: (sample_rate as u64 * 30 / 1000) as u32,
            voiced_grace_count: 0,
            whisper_grace_count: 0,
            initialized: false,
            last_amplitude_db: f32::NEG_INFINITY,
            last_zcr: 0.0,
            last_centroid_hz: 0.0,
            last_is_transient: false,
            // Lookback buffer initialization
            lookback_buffer: vec![0.0; lookback_capacity],
            lookback_write_index: 0,
            lookback_capacity,
            lookback_filled: false,
            lookback_threshold_db: -55.0,
            last_lookback_offset_ms: None,
            last_state_change: SpeechStateChange::None,

            // Word break detection - tuned for OmniRec's longer segments
            word_break_threshold_ratio: 0.5,
            min_word_break_samples: (sample_rate as u64 * 150 / 1000) as u32, // 150ms (vs FlowSTT's 15ms)
            max_word_break_samples: (sample_rate as u64 * 500 / 1000) as u32, // 500ms (vs FlowSTT's 200ms)
            recent_speech_window_samples: (sample_rate as u64 * 100 / 1000) as u32,
            recent_speech_amplitude_sum: 0.0,
            recent_speech_amplitude_count: 0,
            in_word_break: false,
            word_break_sample_count: 0,
            word_break_start_speech_samples: 0,
            last_is_word_break: false,
            last_word_break_event: None,
            total_samples_processed: 0,
            last_debug_log_samples: 0,
        }
    }

    /// Process audio samples and detect speech state changes.
    ///
    /// After calling this method, use `take_state_change()` to get any speech
    /// start/end events, and `take_word_break_event()` to get any word break events.
    pub fn process(&mut self, samples: &[f32]) {
        // Reset state change at start of each process call
        self.last_state_change = SpeechStateChange::None;
        self.last_word_break_event = None;

        // Track samples processed
        self.total_samples_processed += samples.len() as u64;

        // Add samples to lookback buffer (always, for retroactive analysis)
        self.push_to_lookback_buffer(samples);

        // Calculate all features
        let rms = Self::calculate_rms(samples);
        let db = Self::amplitude_to_db(rms);
        let zcr = Self::calculate_zcr(samples);
        let centroid = self.estimate_spectral_centroid(samples, db);

        // Store metrics
        self.last_amplitude_db = db;
        self.last_zcr = zcr;
        self.last_centroid_hz = centroid;
        self.last_is_transient = self.is_transient(zcr, centroid);
        self.last_lookback_offset_ms = None;
        self.last_is_word_break = false;

        // Periodic debug logging (every 5 seconds of audio)
        let log_interval_samples = self.sample_rate as u64 * 5; // 5 seconds
        if self.total_samples_processed - self.last_debug_log_samples >= log_interval_samples {
            self.last_debug_log_samples = self.total_samples_processed;
            let total_secs = self.total_samples_processed as f64 / self.sample_rate as f64;
            eprintln!(
                "[VoiceDetector] {:.1}s processed: dB={:.1}, zcr={:.3}, centroid={:.0}Hz, speaking={}, pending_voiced={}, pending_whisper={}",
                total_secs,
                db,
                zcr,
                centroid,
                self.is_speaking,
                self.is_pending_voiced,
                self.is_pending_whisper
            );
        }

        if !self.initialized {
            self.initialized = true;
            return;
        }

        // Check for transient rejection
        if self.last_is_transient {
            self.reset_onset_state();
            if !self.is_speaking {
                return;
            }
        }

        // Check feature matching for both modes
        let is_voiced = self.matches_voiced_mode(db, zcr, centroid);
        let is_whisper = self.matches_whisper_mode(db, zcr, centroid);
        let is_speech_candidate = is_voiced || is_whisper;

        let samples_len = samples.len() as u32;

        if is_speech_candidate {
            self.silence_sample_count = 0;

            if self.is_speaking {
                // Continue confirmed speech
                self.speech_sample_count += samples.len() as u64;
                self.update_speech_amplitude_average(rms, samples_len);

                // Check if word break ended
                if self.in_word_break {
                    if self.word_break_sample_count >= self.min_word_break_samples
                        && self.word_break_sample_count <= self.max_word_break_samples
                    {
                        let gap_duration_ms =
                            self.samples_to_ms(self.word_break_sample_count as u64) as u32;
                        let offset_ms =
                            self.samples_to_ms(self.word_break_start_speech_samples) as u32;

                        self.last_word_break_event = Some(WordBreakEvent {
                            offset_ms,
                            gap_duration_ms,
                        });

                        tracing::debug!(
                            "Word break detected (offset: {}ms, gap: {}ms)",
                            offset_ms,
                            gap_duration_ms
                        );
                    }
                    self.in_word_break = false;
                    self.word_break_sample_count = 0;
                }
            } else {
                // Handle onset accumulation
                if is_voiced {
                    self.voiced_grace_count = 0;
                    if !self.is_pending_voiced {
                        self.is_pending_voiced = true;
                        self.voiced_onset_count = samples_len;
                    } else {
                        self.voiced_onset_count += samples_len;
                    }

                    if self.voiced_onset_count >= self.voiced_config.onset_samples {
                        self.confirm_speech_start();
                        return;
                    }
                }

                if is_whisper {
                    self.whisper_grace_count = 0;
                    if !self.is_pending_whisper {
                        self.is_pending_whisper = true;
                        self.whisper_onset_count = samples_len;
                    } else {
                        self.whisper_onset_count += samples_len;
                    }

                    if !self.is_speaking
                        && self.whisper_onset_count >= self.whisper_config.onset_samples
                    {
                        self.confirm_speech_start();
                    }
                }
            }
        } else {
            // No speech-like features - use grace period before resetting onset
            if self.is_pending_voiced {
                self.voiced_grace_count += samples_len;
                if self.voiced_grace_count >= self.onset_grace_samples {
                    self.is_pending_voiced = false;
                    self.voiced_onset_count = 0;
                    self.voiced_grace_count = 0;
                }
            }

            if self.is_pending_whisper {
                self.whisper_grace_count += samples_len;
                if self.whisper_grace_count >= self.onset_grace_samples {
                    self.is_pending_whisper = false;
                    self.whisper_onset_count = 0;
                    self.whisper_grace_count = 0;
                }
            }

            if self.is_speaking {
                self.silence_sample_count += samples_len;

                // Word break detection
                let recent_avg = self.get_recent_speech_amplitude();
                let threshold = recent_avg * self.word_break_threshold_ratio;

                if recent_avg > 0.0 && rms < threshold {
                    if !self.in_word_break {
                        self.in_word_break = true;
                        self.word_break_sample_count = samples_len;
                        self.word_break_start_speech_samples = self.speech_sample_count;
                    } else {
                        self.word_break_sample_count += samples_len;
                    }

                    if self.word_break_sample_count >= self.min_word_break_samples
                        && self.word_break_sample_count <= self.max_word_break_samples
                    {
                        self.last_is_word_break = true;
                    }
                }

                // Check if hold time has elapsed
                if self.silence_sample_count >= self.hold_samples {
                    let duration_ms = self.samples_to_ms(self.speech_sample_count);
                    self.is_speaking = false;
                    self.speech_sample_count = 0;
                    self.reset_word_break_state();

                    self.last_state_change = SpeechStateChange::Ended { duration_ms };
                    tracing::debug!("Speech ended (duration: {}ms)", duration_ms);
                }
            }
        }
    }

    /// Get the last speech state change detected during process().
    /// Returns the state change and resets it to None for the next call.
    pub fn take_state_change(&mut self) -> SpeechStateChange {
        std::mem::replace(&mut self.last_state_change, SpeechStateChange::None)
    }

    /// Peek at the last speech state change without resetting it.
    #[allow(dead_code)]
    pub fn peek_state_change(&self) -> &SpeechStateChange {
        &self.last_state_change
    }

    /// Take the last word break event, resetting it to None.
    pub fn take_word_break_event(&mut self) -> Option<WordBreakEvent> {
        self.last_word_break_event.take()
    }

    /// Check if currently speaking
    #[allow(dead_code)]
    pub fn is_speaking(&self) -> bool {
        self.is_speaking
    }

    /// Get current amplitude in dB
    #[allow(dead_code)]
    pub fn amplitude_db(&self) -> f32 {
        self.last_amplitude_db
    }

    /// Get current zero-crossing rate
    #[allow(dead_code)]
    pub fn zcr(&self) -> f32 {
        self.last_zcr
    }

    /// Get current spectral centroid in Hz
    #[allow(dead_code)]
    pub fn centroid_hz(&self) -> f32 {
        self.last_centroid_hz
    }

    /// Reset the detector state
    pub fn reset(&mut self) {
        self.is_speaking = false;
        self.is_pending_voiced = false;
        self.is_pending_whisper = false;
        self.voiced_onset_count = 0;
        self.whisper_onset_count = 0;
        self.silence_sample_count = 0;
        self.speech_sample_count = 0;
        self.voiced_grace_count = 0;
        self.whisper_grace_count = 0;
        self.initialized = false;
        self.lookback_write_index = 0;
        self.lookback_filled = false;
        self.last_state_change = SpeechStateChange::None;
        self.reset_word_break_state();
    }

    // =========================================================================
    // Private methods
    // =========================================================================

    /// Confirm speech start with lookback analysis
    fn confirm_speech_start(&mut self) {
        self.is_speaking = true;
        self.speech_sample_count = self.voiced_onset_count.max(self.whisper_onset_count) as u64;
        self.reset_onset_state();

        let (lookback_samples, lookback_offset_ms) = self.find_lookback_start();
        self.last_lookback_offset_ms = Some(lookback_offset_ms);

        let lookback_sample_count = lookback_samples.len();
        self.last_state_change = SpeechStateChange::Started {
            lookback_samples: lookback_sample_count,
        };

        tracing::debug!(
            "Speech started (lookback: {}ms, {} samples)",
            lookback_offset_ms,
            lookback_sample_count
        );
    }

    /// Calculate RMS amplitude of samples
    fn calculate_rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_squares: f32 = samples.iter().map(|s| s * s).sum();
        (sum_squares / samples.len() as f32).sqrt()
    }

    /// Convert linear amplitude to decibels
    fn amplitude_to_db(amplitude: f32) -> f32 {
        if amplitude <= 0.0 {
            return f32::NEG_INFINITY;
        }
        20.0 * amplitude.log10()
    }

    /// Calculate Zero-Crossing Rate (ZCR) of samples.
    fn calculate_zcr(samples: &[f32]) -> f32 {
        if samples.len() < 2 {
            return 0.0;
        }

        let mut crossings = 0u32;
        for i in 1..samples.len() {
            if (samples[i] >= 0.0) != (samples[i - 1] >= 0.0) {
                crossings += 1;
            }
        }

        crossings as f32 / (samples.len() - 1) as f32
    }

    /// Estimate spectral centroid using first-difference approximation.
    fn estimate_spectral_centroid(&self, samples: &[f32], amplitude_db: f32) -> f32 {
        const CENTROID_GATE_DB: f32 = -55.0;
        if samples.len() < 2 || amplitude_db < CENTROID_GATE_DB {
            return 0.0;
        }

        let mut diff_sum = 0.0f32;
        for i in 1..samples.len() {
            diff_sum += (samples[i] - samples[i - 1]).abs();
        }
        let mean_diff = diff_sum / (samples.len() - 1) as f32;

        let mean_abs: f32 = samples.iter().map(|s| s.abs()).sum::<f32>() / samples.len() as f32;

        if mean_abs < 1e-10 {
            return 0.0;
        }

        self.sample_rate as f32 * mean_diff / (2.0 * mean_abs)
    }

    /// Check if features indicate a transient sound
    fn is_transient(&self, zcr: f32, centroid: f32) -> bool {
        zcr > self.transient_zcr_threshold && centroid > self.transient_centroid_threshold
    }

    /// Check if features match voiced speech mode
    fn matches_voiced_mode(&self, db: f32, zcr: f32, centroid: f32) -> bool {
        db >= self.voiced_config.threshold_db
            && zcr >= self.voiced_config.zcr_range.0
            && zcr <= self.voiced_config.zcr_range.1
            && centroid >= self.voiced_config.centroid_range.0
            && centroid <= self.voiced_config.centroid_range.1
    }

    /// Check if features match whisper speech mode
    fn matches_whisper_mode(&self, db: f32, zcr: f32, centroid: f32) -> bool {
        db >= self.whisper_config.threshold_db
            && zcr >= self.whisper_config.zcr_range.0
            && zcr <= self.whisper_config.zcr_range.1
            && centroid >= self.whisper_config.centroid_range.0
            && centroid <= self.whisper_config.centroid_range.1
    }

    /// Convert sample count to milliseconds
    fn samples_to_ms(&self, samples: u64) -> u64 {
        samples * 1000 / self.sample_rate as u64
    }

    /// Reset all onset tracking state
    fn reset_onset_state(&mut self) {
        self.is_pending_voiced = false;
        self.is_pending_whisper = false;
        self.voiced_onset_count = 0;
        self.whisper_onset_count = 0;
        self.voiced_grace_count = 0;
        self.whisper_grace_count = 0;
    }

    /// Add samples to the lookback ring buffer
    fn push_to_lookback_buffer(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.lookback_buffer[self.lookback_write_index] = sample;
            self.lookback_write_index = (self.lookback_write_index + 1) % self.lookback_capacity;
            if self.lookback_write_index == 0 {
                self.lookback_filled = true;
            }
        }
    }

    /// Get the contents of the lookback buffer in chronological order
    fn get_lookback_buffer_contents(&self) -> Vec<f32> {
        if !self.lookback_filled {
            return self.lookback_buffer[..self.lookback_write_index].to_vec();
        }
        let mut result = Vec::with_capacity(self.lookback_capacity);
        result.extend_from_slice(&self.lookback_buffer[self.lookback_write_index..]);
        result.extend_from_slice(&self.lookback_buffer[..self.lookback_write_index]);
        result
    }

    /// Find the true start of speech by scanning backward through the lookback buffer.
    fn find_lookback_start(&self) -> (Vec<f32>, u32) {
        let buffer = self.get_lookback_buffer_contents();
        if buffer.is_empty() {
            return (Vec::new(), 0);
        }

        const CHUNK_SIZE: usize = 128;
        let margin_samples = (self.sample_rate as usize * 20) / 1000;
        let threshold_linear = 10.0f32.powf(self.lookback_threshold_db / 20.0);

        let mut first_above_threshold_idx = buffer.len();

        let mut pos = buffer.len();
        while pos > 0 {
            let chunk_start = pos.saturating_sub(CHUNK_SIZE);
            let chunk = &buffer[chunk_start..pos];

            let peak = chunk.iter().map(|s| s.abs()).fold(0.0f32, f32::max);

            if peak >= threshold_linear {
                first_above_threshold_idx = chunk_start;
            } else if first_above_threshold_idx < buffer.len() {
                break;
            }

            pos = chunk_start;
        }

        let start_with_margin = first_above_threshold_idx.saturating_sub(margin_samples);
        let lookback_samples = buffer[start_with_margin..].to_vec();
        let samples_before = buffer.len() - start_with_margin;
        let offset_ms = (samples_before as u64 * 1000 / self.sample_rate as u64) as u32;

        (lookback_samples, offset_ms)
    }

    /// Update the running average of speech amplitude
    fn update_speech_amplitude_average(&mut self, rms: f32, sample_count: u32) {
        self.recent_speech_amplitude_sum += rms * sample_count as f32;
        self.recent_speech_amplitude_count += sample_count;

        if self.recent_speech_amplitude_count > self.recent_speech_window_samples {
            let scale = self.recent_speech_window_samples as f32
                / self.recent_speech_amplitude_count as f32;
            self.recent_speech_amplitude_sum *= scale;
            self.recent_speech_amplitude_count = self.recent_speech_window_samples;
        }
    }

    /// Get the recent average speech amplitude
    fn get_recent_speech_amplitude(&self) -> f32 {
        if self.recent_speech_amplitude_count == 0 {
            return 0.0;
        }
        self.recent_speech_amplitude_sum / self.recent_speech_amplitude_count as f32
    }

    /// Reset word break detection state
    fn reset_word_break_state(&mut self) {
        self.in_word_break = false;
        self.word_break_sample_count = 0;
        self.word_break_start_speech_samples = 0;
        self.recent_speech_amplitude_sum = 0.0;
        self.recent_speech_amplitude_count = 0;
        self.last_is_word_break = false;
        self.last_word_break_event = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_rms() {
        let samples = vec![0.5, -0.5, 0.5, -0.5];
        let rms = VoiceDetector::calculate_rms(&samples);
        assert!((rms - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_calculate_zcr() {
        // Alternating samples have maximum ZCR
        let samples = vec![0.5, -0.5, 0.5, -0.5, 0.5];
        let zcr = VoiceDetector::calculate_zcr(&samples);
        assert!((zcr - 1.0).abs() < 0.001);

        // Constant samples have zero ZCR
        let samples = vec![0.5, 0.5, 0.5, 0.5];
        let zcr = VoiceDetector::calculate_zcr(&samples);
        assert!(zcr.abs() < 0.001);
    }

    #[test]
    fn test_amplitude_to_db() {
        // 1.0 amplitude = 0 dB
        assert!((VoiceDetector::amplitude_to_db(1.0) - 0.0).abs() < 0.001);

        // 0.1 amplitude = -20 dB
        assert!((VoiceDetector::amplitude_to_db(0.1) - (-20.0)).abs() < 0.001);

        // 0.0 amplitude = -infinity
        assert!(VoiceDetector::amplitude_to_db(0.0).is_infinite());
    }

    #[test]
    fn test_voice_detector_creation() {
        let detector = VoiceDetector::new(48000);
        assert!(!detector.is_speaking());
        assert!(detector.amplitude_db().is_infinite());
    }
}
