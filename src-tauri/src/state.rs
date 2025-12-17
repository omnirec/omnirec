//! Recording state management.

use crate::capture::{
    get_backend, AudioCaptureBackend, AudioSample, CaptureBackend, CapturedFrame, CaptureRegion,
};
use crate::config::load_config;
use crate::encoder::{encode_frames, encode_frames_with_audio, AudioEncoderConfig};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex, RwLock};

/// Output format for recordings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// MP4 container with H.264 codec (default, no transcoding needed)
    #[default]
    Mp4,
    /// WebM container with VP9 codec
    WebM,
    /// MKV container with H.264 codec (remux only)
    Mkv,
    /// QuickTime container with H.264 codec (remux only)
    QuickTime,
    /// Animated GIF
    Gif,
    /// Animated PNG
    AnimatedPng,
    /// Animated WebP
    AnimatedWebp,
}

impl OutputFormat {
    /// Get the file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Mp4 => "mp4",
            OutputFormat::WebM => "webm",
            OutputFormat::Mkv => "mkv",
            OutputFormat::QuickTime => "mov",
            OutputFormat::Gif => "gif",
            OutputFormat::AnimatedPng => "apng",
            OutputFormat::AnimatedWebp => "webp",
        }
    }

    /// Get display name for this format.
    pub fn display_name(&self) -> &'static str {
        match self {
            OutputFormat::Mp4 => "MP4",
            OutputFormat::WebM => "WebM",
            OutputFormat::Mkv => "MKV",
            OutputFormat::QuickTime => "QuickTime (.mov)",
            OutputFormat::Gif => "GIF",
            OutputFormat::AnimatedPng => "Animated PNG",
            OutputFormat::AnimatedWebp => "Animated WebP",
        }
    }

    /// Parse from string (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "mp4" => Some(OutputFormat::Mp4),
            "webm" => Some(OutputFormat::WebM),
            "mkv" => Some(OutputFormat::Mkv),
            "quicktime" | "mov" => Some(OutputFormat::QuickTime),
            "gif" => Some(OutputFormat::Gif),
            "animatedpng" | "apng" => Some(OutputFormat::AnimatedPng),
            "animatedwebp" | "webp" => Some(OutputFormat::AnimatedWebp),
            _ => None,
        }
    }
}

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
    /// Path to the final output file (transcoded if applicable)
    pub file_path: Option<String>,
    /// Path to the original MP4 source file (same as file_path if format is MP4)
    pub source_path: Option<String>,
    pub error: Option<String>,
}

/// Global recording state manager.
pub struct RecordingManager {
    state: RwLock<RecordingState>,
    stop_flag: Mutex<Option<Arc<AtomicBool>>>,
    audio_stop_flag: Mutex<Option<Arc<AtomicBool>>>,
    recording_start: Mutex<Option<Instant>>,
    encoding_task: Mutex<Option<tokio::task::JoinHandle<Result<PathBuf, String>>>>,
    output_format: RwLock<OutputFormat>,
}

impl RecordingManager {
    /// Create a new recording manager.
    pub fn new() -> Self {
        Self {
            state: RwLock::new(RecordingState::Idle),
            stop_flag: Mutex::new(None),
            audio_stop_flag: Mutex::new(None),
            recording_start: Mutex::new(None),
            encoding_task: Mutex::new(None),
            output_format: RwLock::new(OutputFormat::default()),
        }
    }

    /// Get the current output format.
    pub async fn get_output_format(&self) -> OutputFormat {
        *self.output_format.read().await
    }

    /// Set the output format for future recordings.
    pub async fn set_output_format(&self, format: OutputFormat) -> Result<(), String> {
        let state = self.state.read().await;
        if *state != RecordingState::Idle {
            return Err("Cannot change format while recording".to_string());
        }
        let mut fmt = self.output_format.write().await;
        *fmt = format;
        Ok(())
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

        // Start capture using platform backend
        let backend = get_backend();
        let (frame_rx, stop_flag) = backend
            .start_window_capture(window_handle)
            .map_err(|e| e.to_string())?;

        self.start_encoding(frame_rx, stop_flag).await
    }

    /// Start recording a screen region.
    pub async fn start_region_recording(&self, region: CaptureRegion) -> Result<(), String> {
        // Check current state
        {
            let state = self.state.read().await;
            if *state != RecordingState::Idle {
                return Err("Already recording or saving".to_string());
            }
        }

        // Start region capture using platform backend
        let backend = get_backend();
        let (frame_rx, stop_flag) = backend
            .start_region_capture(region)
            .map_err(|e| e.to_string())?;

        self.start_encoding(frame_rx, stop_flag).await
    }

    /// Start recording an entire display.
    pub async fn start_display_recording(
        &self,
        monitor_id: String,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        // Check current state
        {
            let state = self.state.read().await;
            if *state != RecordingState::Idle {
                return Err("Already recording or saving".to_string());
            }
        }

        // Start display capture using platform backend
        let backend = get_backend();
        let (frame_rx, stop_flag) = backend
            .start_display_capture(monitor_id, width, height)
            .map_err(|e| e.to_string())?;

        self.start_encoding(frame_rx, stop_flag).await
    }

    /// Common encoding startup logic.
    async fn start_encoding(
        &self,
        frame_rx: mpsc::Receiver<CapturedFrame>,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<(), String> {
        // Load audio config
        let config = load_config();
        let audio_enabled = config.audio.enabled && config.audio.source_id.is_some();

        // Store video stop flag
        {
            let mut flag = self.stop_flag.lock().await;
            *flag = Some(stop_flag.clone());
        }

        // Start encoding task (with or without audio)
        let encoding_handle = if audio_enabled {
            // Try to start audio capture
            let audio_source_id = config.audio.source_id.as_ref().unwrap();
            eprintln!(
                "[RecordingManager] Audio enabled, starting capture from source: {}",
                audio_source_id
            );

            match self.start_audio_capture(audio_source_id).await {
                Ok((audio_rx, audio_stop)) => {
                    // Store audio stop flag
                    {
                        let mut flag = self.audio_stop_flag.lock().await;
                        *flag = Some(audio_stop);
                    }

                    // Use default audio encoder config (48kHz stereo)
                    let audio_config = AudioEncoderConfig::default();

                    tokio::spawn(encode_frames_with_audio(
                        frame_rx,
                        audio_rx,
                        stop_flag,
                        audio_config,
                    ))
                }
                Err(e) => {
                    eprintln!(
                        "[RecordingManager] Audio capture failed, recording video only: {}",
                        e
                    );
                    // Fall back to video-only encoding
                    tokio::spawn(encode_frames(frame_rx, stop_flag))
                }
            }
        } else {
            eprintln!("[RecordingManager] Audio disabled, recording video only");
            tokio::spawn(encode_frames(frame_rx, stop_flag))
        };

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

    /// Start audio capture from the specified source.
    async fn start_audio_capture(
        &self,
        source_id: &str,
    ) -> Result<(mpsc::Receiver<AudioSample>, Arc<AtomicBool>), String> {
        let backend = get_backend();
        backend
            .start_audio_capture(source_id)
            .map_err(|e| e.to_string())
    }

    /// Stop the current recording and save the file.
    /// Returns the source MP4 path. Transcoding is handled separately by the caller.
    pub async fn stop_recording(&self) -> Result<(PathBuf, OutputFormat), String> {
        // Check current state
        {
            let state = self.state.read().await;
            if *state != RecordingState::Recording {
                return Err("Not currently recording".to_string());
            }
        }

        // Get the output format before changing state
        let format = self.get_output_format().await;

        // Set state to saving
        {
            let mut state = self.state.write().await;
            *state = RecordingState::Saving;
        }

        // Signal stop (both video and audio)
        {
            let flag = self.stop_flag.lock().await;
            if let Some(ref stop_flag) = *flag {
                stop_flag.store(true, Ordering::Relaxed);
            }
        }
        {
            let flag = self.audio_stop_flag.lock().await;
            if let Some(ref stop_flag) = *flag {
                stop_flag.store(true, Ordering::Relaxed);
            }
        }

        // Wait for encoding to complete
        let source_path = {
            let mut task = self.encoding_task.lock().await;
            if let Some(handle) = task.take() {
                match handle.await {
                    Ok(Ok(path)) => path,
                    Ok(Err(e)) => {
                        self.cleanup().await;
                        return Err(e);
                    }
                    Err(e) => {
                        self.cleanup().await;
                        return Err(format!("Task error: {}", e));
                    }
                }
            } else {
                self.cleanup().await;
                return Err("No encoding task found".to_string());
            }
        };

        // Clean up recording state (but don't reset to idle yet - caller handles that after transcoding)
        {
            let mut flag = self.stop_flag.lock().await;
            *flag = None;
        }
        {
            let mut flag = self.audio_stop_flag.lock().await;
            *flag = None;
        }
        {
            let mut start = self.recording_start.lock().await;
            *start = None;
        }

        Ok((source_path, format))
    }

    /// Clean up internal state and reset to idle.
    async fn cleanup(&self) {
        {
            let mut flag = self.stop_flag.lock().await;
            *flag = None;
        }
        {
            let mut flag = self.audio_stop_flag.lock().await;
            *flag = None;
        }
        {
            let mut start = self.recording_start.lock().await;
            *start = None;
        }
        {
            let mut state = self.state.write().await;
            *state = RecordingState::Idle;
        }
    }

    /// Reset state to idle after recording/transcoding is complete.
    pub async fn set_idle(&self) {
        let mut state = self.state.write().await;
        *state = RecordingState::Idle;
    }
}

impl Default for RecordingManager {
    fn default() -> Self {
        Self::new()
    }
}
