//! Segment ring buffer for continuous audio capture.
//!
//! This module provides a ring buffer for capturing audio during recording,
//! with segment extraction when speech ends or duration thresholds are reached.
//!
//! Ported from FlowSTT's `transcribe_mode.rs` and tuned for OmniRec's longer segments.

/// Ring buffer capacity: 35 seconds at 16kHz mono (whisper's sample rate)
/// 16000 * 35 = 560,000 samples
///
/// 35 seconds is enough for max segment duration (15s) with comfortable margin
const RING_BUFFER_CAPACITY: usize = 16000 * 35;

/// Overflow threshold: 90% of buffer capacity
#[allow(dead_code)]
const OVERFLOW_THRESHOLD_PERCENT: usize = 90;

/// A ring buffer for continuous audio capture during transcription.
///
/// Provides continuous write without blocking, and segment extraction by copying
/// samples between indices. Handles wraparound correctly.
///
/// The buffer stores mono 16kHz audio (resampled from capture format) for
/// direct input to whisper.cpp.
pub struct SegmentRingBuffer {
    /// The underlying buffer
    buffer: Vec<f32>,
    /// Current write position
    write_pos: usize,
    /// Capacity of the buffer
    capacity: usize,
    /// Total samples written (for tracking)
    total_written: u64,
}

impl SegmentRingBuffer {
    /// Create a new ring buffer with the specified capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0.0; capacity],
            write_pos: 0,
            capacity,
            total_written: 0,
        }
    }

    /// Create a ring buffer with default capacity (35 seconds at 48kHz)
    pub fn with_default_capacity() -> Self {
        Self::new(RING_BUFFER_CAPACITY)
    }

    /// Write samples to the buffer, advancing write position and wrapping
    pub fn write(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.buffer[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % self.capacity;
            self.total_written += 1;
        }
    }

    /// Get current write position
    #[allow(dead_code)]
    pub fn write_position(&self) -> usize {
        self.write_pos
    }

    /// Get buffer capacity
    #[allow(dead_code)]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get total samples written since creation/clear
    #[allow(dead_code)]
    pub fn total_written(&self) -> u64 {
        self.total_written
    }

    /// Calculate segment length from start_idx to current write_pos, handling wraparound
    #[allow(dead_code)]
    pub fn segment_length(&self, start_idx: usize) -> usize {
        if self.write_pos >= start_idx {
            self.write_pos - start_idx
        } else {
            // Wraparound case: distance from start to end + distance from 0 to write_pos
            (self.capacity - start_idx) + self.write_pos
        }
    }

    /// Calculate a sample index from lookback offset (samples back from write_pos)
    pub fn index_from_lookback(&self, lookback_samples: usize) -> usize {
        if lookback_samples >= self.capacity {
            // Clamp to buffer size
            self.write_pos
        } else if lookback_samples <= self.write_pos {
            self.write_pos - lookback_samples
        } else {
            // Wraparound case
            self.capacity - (lookback_samples - self.write_pos)
        }
    }

    /// Check if segment length exceeds overflow threshold
    #[allow(dead_code)]
    pub fn is_approaching_overflow(&self, start_idx: usize) -> bool {
        let segment_len = self.segment_length(start_idx);
        let threshold = (self.capacity * OVERFLOW_THRESHOLD_PERCENT) / 100;
        segment_len >= threshold
    }

    /// Extract segment from start_idx to current write_pos, handling wraparound
    /// Returns a new Vec with the copied samples
    pub fn extract_segment(&self, start_idx: usize) -> Vec<f32> {
        self.extract_segment_to(start_idx, self.write_pos)
    }

    /// Extract segment from start_idx to a specific end_idx, handling wraparound
    /// Returns a new Vec with the copied samples
    pub fn extract_segment_to(&self, start_idx: usize, end_idx: usize) -> Vec<f32> {
        // Calculate segment length handling wraparound
        let segment_len = if end_idx >= start_idx {
            end_idx - start_idx
        } else {
            (self.capacity - start_idx) + end_idx
        };

        if segment_len == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(segment_len);

        if end_idx >= start_idx {
            // No wraparound: simple slice copy
            result.extend_from_slice(&self.buffer[start_idx..end_idx]);
        } else {
            // Wraparound: copy from start_idx to end, then from 0 to end_idx
            result.extend_from_slice(&self.buffer[start_idx..]);
            result.extend_from_slice(&self.buffer[..end_idx]);
        }

        result
    }

    /// Clear the buffer (reset write position but don't zero memory)
    pub fn clear(&mut self) {
        self.write_pos = 0;
        self.total_written = 0;
    }
}

impl Default for SegmentRingBuffer {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_and_extract() {
        let mut buffer = SegmentRingBuffer::new(100);

        // Write some samples
        buffer.write(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(buffer.write_position(), 5);
        assert_eq!(buffer.total_written(), 5);

        // Extract from start
        let segment = buffer.extract_segment(0);
        assert_eq!(segment, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn test_segment_length() {
        let mut buffer = SegmentRingBuffer::new(100);
        buffer.write(&[1.0; 50]);

        assert_eq!(buffer.segment_length(0), 50);
        assert_eq!(buffer.segment_length(25), 25);
        assert_eq!(buffer.segment_length(50), 0);
    }

    #[test]
    fn test_wraparound() {
        let mut buffer = SegmentRingBuffer::new(10);

        // Fill buffer and wrap around
        buffer.write(&[1.0; 8]); // Position at 8
        buffer.write(&[2.0; 5]); // Position at 3 (wrapped)

        assert_eq!(buffer.write_position(), 3);

        // Extract from position 5 to current (3) - should wrap
        let segment = buffer.extract_segment(5);
        assert_eq!(segment.len(), 8); // 5 from old + 3 from new
    }

    #[test]
    fn test_index_from_lookback() {
        let mut buffer = SegmentRingBuffer::new(100);
        buffer.write(&[1.0; 50]);

        // Lookback within current segment
        assert_eq!(buffer.index_from_lookback(10), 40);
        assert_eq!(buffer.index_from_lookback(50), 0);

        // Lookback at exactly write position
        assert_eq!(buffer.index_from_lookback(0), 50);
    }

    #[test]
    fn test_overflow_detection() {
        let mut buffer = SegmentRingBuffer::new(100);

        // Fill to 85% - not overflow
        buffer.write(&[1.0; 85]);
        assert!(!buffer.is_approaching_overflow(0));

        // Fill to 92% - overflow
        buffer.write(&[1.0; 7]);
        assert!(buffer.is_approaching_overflow(0));
    }

    #[test]
    fn test_clear() {
        let mut buffer = SegmentRingBuffer::new(100);
        buffer.write(&[1.0; 50]);
        buffer.clear();

        assert_eq!(buffer.write_position(), 0);
        assert_eq!(buffer.total_written(), 0);
    }
}
