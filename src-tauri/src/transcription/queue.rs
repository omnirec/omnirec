//! Transcription queue for async segment processing.
//!
//! This module provides a bounded queue for audio segments awaiting transcription,
//! with a worker thread that processes segments sequentially.
//!
//! Ported from FlowSTT's `transcribe_mode.rs` and adapted for OmniRec's
//! file-based transcript output.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::transcriber::Transcriber;
use super::transcript_writer::TranscriptWriter;

/// Callback type for when a transcription segment is produced.
/// Parameters: (timestamp_secs, text)
pub type OnSegmentCallback = Box<dyn Fn(f64, String) + Send + Sync + 'static>;

/// Maximum queue size for transcription segments
const MAX_QUEUE_SIZE: usize = 10;

/// A segment queued for transcription
pub struct QueuedSegment {
    /// Audio samples (mono 16kHz f32, ready for whisper)
    pub samples: Vec<f32>,
    /// Timestamp when the segment started (seconds from recording start)
    pub timestamp_secs: f64,
}

/// A bounded queue for audio segments awaiting transcription.
///
/// Provides thread-safe enqueue/dequeue operations with a worker thread
/// that processes segments sequentially and writes results to a transcript file.
pub struct TranscriptionQueue {
    /// The queue of segments
    queue: Arc<Mutex<VecDeque<QueuedSegment>>>,
    /// Flag indicating worker should continue running
    worker_active: Arc<AtomicBool>,
    /// Count of segments currently in queue
    queue_count: Arc<AtomicUsize>,
    /// Total segments processed
    segments_processed: Arc<AtomicUsize>,
}

impl TranscriptionQueue {
    /// Create a new transcription queue
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            worker_active: Arc::new(AtomicBool::new(false)),
            queue_count: Arc::new(AtomicUsize::new(0)),
            segments_processed: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Enqueue a segment for transcription.
    /// Returns false if queue is full (segment was not added).
    pub fn enqueue(&self, segment: QueuedSegment) -> bool {
        let mut queue = self.queue.lock().unwrap();
        if queue.len() >= MAX_QUEUE_SIZE {
            tracing::warn!("Transcription queue is full, segment dropped");
            return false;
        }
        queue.push_back(segment);
        self.queue_count.store(queue.len(), Ordering::SeqCst);
        true
    }

    /// Get current queue depth
    pub fn queue_depth(&self) -> usize {
        self.queue_count.load(Ordering::SeqCst)
    }

    /// Get total segments processed
    pub fn segments_processed(&self) -> usize {
        self.segments_processed.load(Ordering::SeqCst)
    }

    /// Check if worker is active
    #[allow(dead_code)]
    pub fn is_worker_active(&self) -> bool {
        self.worker_active.load(Ordering::SeqCst)
    }

    /// Start the transcription worker thread
    ///
    /// # Arguments
    /// * `transcript_path` - Path to the transcript markdown file
    /// * `model_path` - Optional custom model path (uses default if None)
    #[allow(dead_code)] // Used by start_worker_with_callback; kept for API completeness
    pub fn start_worker(&self, transcript_path: PathBuf, model_path: Option<PathBuf>) {
        self.start_worker_with_callback(transcript_path, model_path, None);
    }

    /// Start the transcription worker thread with an optional callback for segment events.
    ///
    /// # Arguments
    /// * `transcript_path` - Path to the transcript markdown file
    /// * `model_path` - Optional custom model path (uses default if None)
    /// * `on_segment` - Optional callback invoked when a segment is transcribed
    pub fn start_worker_with_callback(
        &self,
        transcript_path: PathBuf,
        model_path: Option<PathBuf>,
        on_segment: Option<Arc<OnSegmentCallback>>,
    ) {
        if self.worker_active.load(Ordering::SeqCst) {
            return; // Already running
        }

        self.worker_active.store(true, Ordering::SeqCst);
        self.segments_processed.store(0, Ordering::SeqCst);

        let queue = Arc::clone(&self.queue);
        let worker_active = Arc::clone(&self.worker_active);
        let queue_count = Arc::clone(&self.queue_count);
        let segments_processed = Arc::clone(&self.segments_processed);

        thread::spawn(move || {
            eprintln!(
                "[TranscriptionQueue] Worker thread starting, transcript: {:?}",
                transcript_path
            );

            // Initialize transcriber
            let mut transcriber = match model_path {
                Some(path) => Transcriber::with_model_path(path),
                None => Transcriber::new(),
            };

            // Initialize transcript writer
            let mut writer = match TranscriptWriter::new(&transcript_path) {
                Ok(w) => {
                    eprintln!("[TranscriptionQueue] Transcript writer created successfully");
                    w
                }
                Err(e) => {
                    eprintln!(
                        "[TranscriptionQueue] Failed to create transcript writer: {}",
                        e
                    );
                    worker_active.store(false, Ordering::SeqCst);
                    return;
                }
            };

            // Try to pre-load model
            if transcriber.is_model_available() {
                eprintln!("[TranscriptionQueue] Model available, pre-loading...");
                if let Err(e) = transcriber.load_model() {
                    eprintln!(
                        "[TranscriptionQueue] Failed to pre-load whisper model: {}",
                        e
                    );
                } else {
                    eprintln!("[TranscriptionQueue] Model loaded successfully");
                }
            } else {
                eprintln!("[TranscriptionQueue] Whisper model not available");
            }

            loop {
                // Check if we should stop
                if !worker_active.load(Ordering::SeqCst) {
                    // Drain remaining queue before exiting
                    let remaining = {
                        let q = queue.lock().unwrap();
                        q.len()
                    };
                    if remaining == 0 {
                        break;
                    }
                    // Continue processing remaining items
                }

                // Try to get a segment from queue
                let segment = {
                    let mut q = queue.lock().unwrap();
                    let seg = q.pop_front();
                    queue_count.store(q.len(), Ordering::SeqCst);
                    seg
                };

                match segment {
                    Some(seg) => {
                        let duration_secs = seg.samples.len() as f64 / 16000.0;
                        eprintln!(
                            "[TranscriptionQueue] Processing segment: {:.2}s ({} samples) at {:.1}s",
                            duration_secs,
                            seg.samples.len(),
                            seg.timestamp_secs
                        );

                        // Transcribe the segment
                        match transcriber.transcribe(&seg.samples) {
                            Ok(text) => {
                                if !text.is_empty() {
                                    eprintln!(
                                        "[TranscriptionQueue] Transcribed: \"{}\"",
                                        text.chars().take(80).collect::<String>()
                                    );
                                    // Write to transcript file
                                    if let Err(e) = writer.write_segment(seg.timestamp_secs, &text)
                                    {
                                        eprintln!(
                                            "[TranscriptionQueue] Failed to write transcript: {}",
                                            e
                                        );
                                    }
                                    // Invoke callback if provided
                                    if let Some(ref callback) = on_segment {
                                        callback(seg.timestamp_secs, text);
                                    }
                                } else {
                                    eprintln!(
                                        "[TranscriptionQueue] Transcription returned empty text"
                                    );
                                }
                            }
                            Err(e) => {
                                eprintln!("[TranscriptionQueue] Transcription failed: {}", e);
                            }
                        }

                        segments_processed.fetch_add(1, Ordering::SeqCst);
                    }
                    None => {
                        // No segment available, sleep briefly
                        thread::sleep(Duration::from_millis(50));
                    }
                }
            }

            // Finalize transcript
            if let Err(e) = writer.finalize() {
                tracing::error!("Failed to finalize transcript: {}", e);
            }

            tracing::info!("Transcription worker thread exiting");
        });
    }

    /// Stop the transcription worker (will drain remaining queue)
    pub fn stop_worker(&self) {
        self.worker_active.store(false, Ordering::SeqCst);
    }

    /// Clear the queue (discard pending segments)
    #[allow(dead_code)]
    pub fn clear(&self) {
        let mut queue = self.queue.lock().unwrap();
        queue.clear();
        self.queue_count.store(0, Ordering::SeqCst);
    }
}

impl Default for TranscriptionQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_creation() {
        let queue = TranscriptionQueue::new();
        assert_eq!(queue.queue_depth(), 0);
        assert!(!queue.is_worker_active());
    }

    #[test]
    fn test_enqueue() {
        let queue = TranscriptionQueue::new();

        let segment = QueuedSegment {
            samples: vec![0.0; 1000],
            timestamp_secs: 0.0,
        };

        assert!(queue.enqueue(segment));
        assert_eq!(queue.queue_depth(), 1);
    }

    #[test]
    fn test_queue_limit() {
        let queue = TranscriptionQueue::new();

        // Fill the queue
        for i in 0..MAX_QUEUE_SIZE {
            let segment = QueuedSegment {
                samples: vec![0.0; 100],
                timestamp_secs: i as f64,
            };
            assert!(queue.enqueue(segment));
        }

        // Next enqueue should fail
        let segment = QueuedSegment {
            samples: vec![0.0; 100],
            timestamp_secs: 999.0,
        };
        assert!(!queue.enqueue(segment));
    }
}
