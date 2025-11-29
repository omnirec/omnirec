//! Recording state management.

use crate::capture::recorder::start_capture;
use crate::encoder::encode_frames;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};

/// Recording state enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordingState {
    /// Not recording, ready to start
    Idle,
    /// Currently recording
    Recording,
    /// Finalizing the recording (encoding remaining frames, writing file)
    Saving,
}

/// Result of a completed recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingResult {
    pub success: bool,
    pub file_path: Option<String>,
    pub error: Option<String>,
}

/// Global recording state manager.
pub struct RecordingManager {
    state: RwLock<RecordingState>,
    stop_flag: Mutex<Option<Arc<AtomicBool>>>,
    recording_start: Mutex<Option<Instant>>,
    encoding_task: Mutex<Option<tokio::task::JoinHandle<Result<PathBuf, String>>>>,
}

impl RecordingManager {
    /// Create a new recording manager.
    pub fn new() -> Self {
        Self {
            state: RwLock::new(RecordingState::Idle),
            stop_flag: Mutex::new(None),
            recording_start: Mutex::new(None),
            encoding_task: Mutex::new(None),
        }
    }

    /// Get the current recording state.
    pub async fn get_state(&self) -> RecordingState {
        *self.state.read().await
    }

    /// Get elapsed recording time in seconds.
    pub async fn get_elapsed_seconds(&self) -> u64 {
        let start = self.recording_start.lock().await;
        match *start {
            Some(instant) => instant.elapsed().as_secs(),
            None => 0,
        }
    }

    /// Start recording the specified window.
    pub async fn start_recording(&self, window_handle: isize) -> Result<(), String> {
        // Check current state
        {
            let state = self.state.read().await;
            if *state != RecordingState::Idle {
                return Err("Already recording or saving".to_string());
            }
        }

        // Start capture
        let (frame_rx, stop_flag) = start_capture(window_handle)?;

        // Store stop flag
        {
            let mut flag = self.stop_flag.lock().await;
            *flag = Some(stop_flag.clone());
        }

        // Start encoding task
        let encoding_handle = tokio::spawn(encode_frames(frame_rx, stop_flag));

        {
            let mut task = self.encoding_task.lock().await;
            *task = Some(encoding_handle);
        }

        // Update state
        {
            let mut state = self.state.write().await;
            *state = RecordingState::Recording;
        }

        // Record start time
        {
            let mut start = self.recording_start.lock().await;
            *start = Some(Instant::now());
        }

        Ok(())
    }

    /// Stop the current recording and save the file.
    pub async fn stop_recording(&self) -> Result<RecordingResult, String> {
        // Check current state
        {
            let state = self.state.read().await;
            if *state != RecordingState::Recording {
                return Err("Not currently recording".to_string());
            }
        }

        // Set state to saving
        {
            let mut state = self.state.write().await;
            *state = RecordingState::Saving;
        }

        // Signal stop
        {
            let flag = self.stop_flag.lock().await;
            if let Some(ref stop_flag) = *flag {
                stop_flag.store(true, Ordering::Relaxed);
            }
        }

        // Wait for encoding to complete
        let result = {
            let mut task = self.encoding_task.lock().await;
            if let Some(handle) = task.take() {
                match handle.await {
                    Ok(Ok(path)) => RecordingResult {
                        success: true,
                        file_path: Some(path.to_string_lossy().to_string()),
                        error: None,
                    },
                    Ok(Err(e)) => RecordingResult {
                        success: false,
                        file_path: None,
                        error: Some(e),
                    },
                    Err(e) => RecordingResult {
                        success: false,
                        file_path: None,
                        error: Some(format!("Task error: {}", e)),
                    },
                }
            } else {
                RecordingResult {
                    success: false,
                    file_path: None,
                    error: Some("No encoding task found".to_string()),
                }
            }
        };

        // Clean up
        {
            let mut flag = self.stop_flag.lock().await;
            *flag = None;
        }
        {
            let mut start = self.recording_start.lock().await;
            *start = None;
        }

        // Reset state to idle
        {
            let mut state = self.state.write().await;
            *state = RecordingState::Idle;
        }

        Ok(result)
    }
}

impl Default for RecordingManager {
    fn default() -> Self {
        Self::new()
    }
}
