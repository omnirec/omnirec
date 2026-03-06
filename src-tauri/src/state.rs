//! Recording state management for the OmniRec service.
//!
//! This module manages the recording lifecycle, including:
//! - Recording state (idle, recording, saving)
//! - Output format configuration
//! - Audio configuration
//! - Transcription configuration (delegated to vtx-engine)
//! - Elapsed time tracking
//! - Event broadcasting to subscribed clients

use crate::capture::{
    AudioCaptureBackend, AudioReceiver, CaptureBackend, CaptureRegion, FrameReceiver, StopHandle,
};
use crate::encoder::{
    encode_frames, encode_frames_with_audio, encode_frames_with_audio_and_transcription,
    AudioEncoderConfig,
};
use omnirec_types::{
    AudioConfig, OutputFormat, RecordingState, TranscriptionConfig, TranscriptionSegment,
    TranscriptionStatus,
};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::{error, info, warn};
use vtx_common::EngineEvent;
use vtx_engine::{AudioEngine, EngineBuilder};

/// Result of a completed recording.
#[derive(Debug, Clone)]
pub struct RecordingResult {
    /// Path to the source MP4 file
    pub source_path: PathBuf,
    /// Path to the final output file (after transcoding, if applicable)
    pub file_path: PathBuf,
}

/// Events broadcast to subscribed clients.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used for event serialization, not accessed directly
pub enum ServiceEvent {
    /// Recording state changed
    StateChanged(RecordingState),
    /// Elapsed time update (sent every second during recording)
    ElapsedTime(u64),
    /// Transcoding started
    TranscodingStarted { format: String },
    /// Transcoding completed
    TranscodingComplete { success: bool, path: Option<String> },
    /// A transcription segment was produced
    TranscriptionSegment { timestamp_secs: f64, text: String },
    /// Service is shutting down
    Shutdown,
}

/// Thin transcript writer: creates the file, writes the heading and appends segments.
struct TranscriptWriter {
    writer: BufWriter<std::fs::File>,
}

impl TranscriptWriter {
    /// Create a new transcript writer at the given path.
    fn new(path: &PathBuf) -> Result<Self, std::io::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        writeln!(writer, "# Recording Transcript\n")?;
        writer.flush()?;
        Ok(Self { writer })
    }

    /// Derive transcript path from the video output path.
    fn transcript_path(video_path: &Path) -> PathBuf {
        let stem = video_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "recording".to_string());
        let parent = video_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        parent.join(format!("{}_transcript.md", stem))
    }

    /// Append a transcription segment line: `[HH:MM:SS] text`.
    fn append_segment(&mut self, timestamp_offset_ms: u64, text: &str) {
        let total_secs = timestamp_offset_ms / 1000;
        let h = total_secs / 3600;
        let m = (total_secs % 3600) / 60;
        let s = total_secs % 60;
        let line = format!("[{:02}:{:02}:{:02}] {}\n\n", h, m, s, text);
        let _ = self.writer.write_all(line.as_bytes());
        let _ = self.writer.flush();
    }
}

/// Global recording state manager for the service.
pub struct RecordingManager {
    state: RwLock<RecordingState>,
    stop_flag: Mutex<Option<StopHandle>>,
    audio_stop_flag: Mutex<Option<StopHandle>>,
    recording_start: Mutex<Option<Instant>>,
    encoding_task: Mutex<Option<tokio::task::JoinHandle<Result<PathBuf, String>>>>,
    output_format: RwLock<OutputFormat>,
    audio_config: RwLock<AudioConfig>,
    /// Transcription configuration (enabled/model path)
    transcription_config: RwLock<TranscriptionConfig>,
    /// vtx-engine instance for audio capture and transcription.
    /// Wrapped in Arc so it can be shared with the event subscriber task.
    engine: Arc<AudioEngine>,
    /// Handle for the engine event subscriber task
    engine_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Broadcast channel for events
    event_tx: broadcast::Sender<ServiceEvent>,
    /// Elapsed time update task handle
    elapsed_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Transcription segments accumulated during the current recording session.
    /// Uses Arc<std::sync::Mutex> because the OnceLock init is synchronous and
    /// we want to avoid async in the accessor path.
    transcription_segments: std::sync::Arc<std::sync::Mutex<Vec<TranscriptionSegment>>>,
}

impl RecordingManager {
    /// Create a new recording manager.
    ///
    /// # Panics
    /// Panics if the vtx-engine fails to initialize (should not happen in normal use).
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(100);

        // Build vtx-engine with the OmniRec long-form transcription profile.
        // We spawn a temporary single-thread runtime because RecordingManager::new()
        // is called from a synchronous OnceLock initializer.
        let engine = {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime for engine init");
            rt.block_on(async {
                EngineBuilder::new()
                    .app_name("OmniRec")
                    .with_profile(vtx_common::TranscriptionProfile::Transcription)
                    // VAD thresholds preserved from the previous OmniRec transcription module
                    .vad_voiced_threshold_db(-42.0)
                    .vad_whisper_threshold_db(-52.0)
                    .vad_voiced_onset_ms(80)
                    .vad_whisper_onset_ms(120)
                    .without_visualization()
                    .build()
                    .await
                    .expect("Failed to initialize vtx-engine")
                    .0
            })
        };

        Self {
            state: RwLock::new(RecordingState::Idle),
            stop_flag: Mutex::new(None),
            audio_stop_flag: Mutex::new(None),
            recording_start: Mutex::new(None),
            encoding_task: Mutex::new(None),
            output_format: RwLock::new(OutputFormat::default()),
            audio_config: RwLock::new(AudioConfig::default()),
            transcription_config: RwLock::new(TranscriptionConfig::default()),
            engine: Arc::new(engine),
            engine_task: Mutex::new(None),
            event_tx,
            elapsed_task: Mutex::new(None),
            transcription_segments: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// Subscribe to service events.
    #[allow(dead_code)]
    pub fn subscribe(&self) -> broadcast::Receiver<ServiceEvent> {
        self.event_tx.subscribe()
    }

    /// Broadcast an event to all subscribers.
    fn broadcast(&self, event: ServiceEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Get the current recording state.
    pub async fn get_state(&self) -> RecordingState {
        *self.state.read().await
    }

    /// Set the recording state and broadcast the change.
    async fn set_state(&self, new_state: RecordingState) {
        {
            let mut state = self.state.write().await;
            *state = new_state;
        }
        self.broadcast(ServiceEvent::StateChanged(new_state));
    }

    /// Get elapsed recording time in seconds.
    pub async fn get_elapsed_seconds(&self) -> u64 {
        let start = self.recording_start.lock().await;
        match *start {
            Some(instant) => instant.elapsed().as_secs(),
            None => 0,
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
        info!("Output format set to {:?}", format);
        Ok(())
    }

    /// Get the current audio configuration.
    pub async fn get_audio_config(&self) -> AudioConfig {
        self.audio_config.read().await.clone()
    }

    /// Set the audio configuration.
    pub async fn set_audio_config(&self, config: AudioConfig) -> Result<(), String> {
        let state = self.state.read().await;
        if *state != RecordingState::Idle {
            return Err("Cannot change audio config while recording".to_string());
        }
        let mut cfg = self.audio_config.write().await;
        *cfg = config;
        info!("Audio configuration updated");
        Ok(())
    }

    /// Get the current transcription configuration.
    pub async fn get_transcription_config(&self) -> TranscriptionConfig {
        self.transcription_config.read().await.clone()
    }

    /// Set the transcription configuration.
    pub async fn set_transcription_config(
        &self,
        config: TranscriptionConfig,
    ) -> Result<(), String> {
        let state = self.state.read().await;
        if *state != RecordingState::Idle {
            return Err("Cannot change transcription config while recording".to_string());
        }
        let enabled = config.enabled;
        let mut cfg = self.transcription_config.write().await;
        *cfg = config;
        info!("Transcription configuration updated: enabled={}", enabled);
        Ok(())
    }

    /// Get current transcription status (from vtx-engine state).
    pub async fn get_transcription_status(&self) -> TranscriptionStatus {
        let engine_status = self.engine.get_status();
        let model_status = self.engine.check_model_status();
        let recording_active = self.engine.is_recording();

        TranscriptionStatus {
            model_loaded: model_status.available,
            active: recording_active || engine_status.capturing,
            segments_processed: {
                let segs = self.transcription_segments.lock().unwrap();
                segs.len() as u32
            },
            queue_depth: engine_status.queue_depth as u32,
            error: None,
        }
    }

    /// Get transcription segments since a given index.
    ///
    /// Returns segments starting from `since_index` and the total count.
    /// Pass 0 to get all segments.
    pub fn get_transcription_segments(&self, since_index: u32) -> (Vec<TranscriptionSegment>, u32) {
        let segments = self.transcription_segments.lock().unwrap();
        let total = segments.len() as u32;
        let from = since_index as usize;

        if from >= segments.len() {
            (Vec::new(), total)
        } else {
            (segments[from..].to_vec(), total)
        }
    }

    /// Clear all transcription segments (called when starting a new recording).
    pub fn clear_transcription_segments(&self) {
        let mut segments = self.transcription_segments.lock().unwrap();
        segments.clear();
    }

    /// Get a clone of the stop flag (for external stop monitoring).
    #[allow(dead_code)]
    pub async fn get_stop_flag(&self) -> Option<StopHandle> {
        self.stop_flag.lock().await.clone()
    }

    /// Start recording the specified window.
    pub async fn start_window_capture(&self, window_handle: isize) -> Result<(), String> {
        self.check_idle().await?;

        let backend = crate::capture::get_backend();
        let (frame_rx, stop_flag) = backend
            .start_window_capture(window_handle)
            .map_err(|e| e.to_string())?;

        self.start_encoding(frame_rx, stop_flag).await
    }

    /// Start recording a screen region.
    pub async fn start_region_capture(&self, region: CaptureRegion) -> Result<(), String> {
        self.check_idle().await?;

        let backend = crate::capture::get_backend();
        let (frame_rx, stop_flag) = backend
            .start_region_capture(region)
            .map_err(|e| e.to_string())?;

        self.start_encoding(frame_rx, stop_flag).await
    }

    /// Start recording an entire display.
    pub async fn start_display_capture(
        &self,
        monitor_id: String,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        self.check_idle().await?;

        let backend = crate::capture::get_backend();
        let (frame_rx, stop_flag) = backend
            .start_display_capture(monitor_id, width, height)
            .map_err(|e| e.to_string())?;

        self.start_encoding(frame_rx, stop_flag).await
    }

    /// Start portal-based capture (for GNOME/KDE tray mode).
    #[cfg(target_os = "linux")]
    pub async fn start_portal_capture(&self) -> Result<(), String> {
        self.check_idle().await?;

        let backend = crate::capture::get_backend();
        let (frame_rx, stop_flag) = backend.start_portal_capture().map_err(|e| e.to_string())?;

        self.start_encoding(frame_rx, stop_flag).await
    }

    #[cfg(not(target_os = "linux"))]
    pub async fn start_portal_capture(&self) -> Result<(), String> {
        Err("Portal capture is only available on Linux".to_string())
    }

    /// Check that we're in idle state.
    async fn check_idle(&self) -> Result<(), String> {
        let state = self.state.read().await;
        if *state != RecordingState::Idle {
            return Err("Already recording or saving".to_string());
        }
        Ok(())
    }

    /// Common encoding startup logic.
    async fn start_encoding(
        &self,
        frame_rx: FrameReceiver,
        stop_flag: StopHandle,
    ) -> Result<(), String> {
        // Clear any previous transcription segments
        self.clear_transcription_segments();

        // Get audio config
        let audio_cfg = self.get_audio_config().await;
        let has_system_audio = audio_cfg.enabled && audio_cfg.source_id.is_some();
        let has_microphone = audio_cfg.enabled && audio_cfg.microphone_id.is_some();
        let audio_enabled = has_system_audio || has_microphone;

        // Get transcription config
        let transcription_cfg = self.get_transcription_config().await;
        // Transcription is enabled when configured and we have some audio source.
        // vtx-engine captures its own audio stream independently of FFmpeg.
        let transcription_enabled = transcription_cfg.enabled && audio_enabled;

        tracing::debug!(
            "[Recording] Audio config: enabled={}, source_id={:?}, mic_id={:?}, has_system_audio={}, has_mic={}, audio_enabled={}",
            audio_cfg.enabled, audio_cfg.source_id, audio_cfg.microphone_id, has_system_audio, has_microphone, audio_enabled
        );
        tracing::debug!(
            "[Recording] Transcription config: enabled={}, transcription_enabled={}",
            transcription_cfg.enabled, transcription_enabled
        );

        // Store video stop flag
        {
            let mut flag = self.stop_flag.lock().await;
            *flag = Some(stop_flag.clone());
        }

        // Start encoding task (with or without audio)
        let encoding_handle = if audio_enabled {
            info!(
                "Starting recording with audio - system: {:?}, mic: {:?}, AEC: {}, transcription: {}",
                audio_cfg.source_id, audio_cfg.microphone_id, audio_cfg.echo_cancellation, transcription_enabled
            );

            match self
                .start_audio_capture_dual(
                    audio_cfg.source_id.as_deref(),
                    audio_cfg.microphone_id.as_deref(),
                    audio_cfg.echo_cancellation,
                )
                .await
            {
                Ok((audio_rx, audio_stop)) => {
                    {
                        let mut flag = self.audio_stop_flag.lock().await;
                        *flag = Some(audio_stop);
                    }

                    let audio_encoder_config = AudioEncoderConfig::default();

                    if transcription_enabled {
                        // Generate output path upfront so the transcript can use the same base name
                        let video_output_path = match crate::encoder::generate_output_path() {
                            Ok(path) => path,
                            Err(e) => {
                                error!("Failed to generate output path: {}", e);
                                return Err(e);
                            }
                        };
                        tracing::debug!("[Transcription] Video output path: {:?}", video_output_path);

                        // Determine the device ID to pass to vtx-engine.
                        // Prefer the microphone device; fall back to system audio source.
                        let mic_device_id = audio_cfg.microphone_id.clone()
                            .or_else(|| audio_cfg.source_id.clone());

                        // Start vtx-engine capture and recording session.
                        self.start_engine_recording(mic_device_id, video_output_path.clone()).await;

                        // Use the existing encoder that supports a pre-generated output path.
                        // No transcription channel needed — vtx-engine handles transcription.
                        tokio::spawn(encode_frames_with_audio_and_transcription(
                            frame_rx,
                            audio_rx,
                            stop_flag.clone(),
                            audio_encoder_config,
                            None, // No transcription channel; vtx-engine captures independently
                            Some(video_output_path),
                        ))
                    } else {
                        tracing::debug!("[Transcription] Transcription is DISABLED");
                        tokio::spawn(encode_frames_with_audio(
                            frame_rx,
                            audio_rx,
                            stop_flag.clone(),
                            audio_encoder_config,
                        ))
                    }
                }
                Err(e) => {
                    warn!("Audio capture failed, recording video only: {}", e);
                    tokio::spawn(encode_frames(frame_rx, stop_flag.clone()))
                }
            }
        } else {
            info!("Starting video-only recording");
            tokio::spawn(encode_frames(frame_rx, stop_flag.clone()))
        };

        {
            let mut task = self.encoding_task.lock().await;
            *task = Some(encoding_handle);
        }

        // Record start time
        {
            let mut start = self.recording_start.lock().await;
            *start = Some(Instant::now());
        }

        // Update state
        self.set_state(RecordingState::Recording).await;

        // Start elapsed time broadcasting
        self.start_elapsed_broadcast().await;

        info!("Recording started");
        Ok(())
    }

    /// Start vtx-engine audio capture and manual recording session.
    ///
    /// # Platform notes (tasks 6.1–6.3)
    /// On Windows (WASAPI), macOS (CoreAudio), and Linux (PipeWire), opening a
    /// second concurrent capture session on the same device alongside FFmpeg's
    /// encoding pipeline is supported — each session receives its own independent
    /// audio stream from the OS audio subsystem. vtx-engine's native capture
    /// backend is therefore used directly here (`start_capture` + `start_recording`)
    /// rather than feeding OmniRec's resampled pipeline through
    /// `transcribe_audio_stream`. This avoids the need for OmniRec to resample
    /// to 16 kHz and route samples into the engine.
    async fn start_engine_recording(
        &self,
        mic_device_id: Option<String>,
        video_output_path: PathBuf,
    ) {
        let engine = self.engine.clone();
        let event_tx = self.event_tx.clone();
        let segments_storage = self.transcription_segments.clone();

        // Subscribe to engine events BEFORE starting capture so we don't miss any events.
        let mut engine_rx = engine.subscribe();

        // Start vtx-engine's audio capture on the selected device.
        if let Err(e) = engine.start_capture(mic_device_id, None).await {
            error!("[Transcription] Engine start_capture failed: {}", e);
            return;
        }

        // Begin the manual recording session (audio accumulation starts here).
        engine.start_recording();
        info!("[Transcription] vtx-engine recording started");

        // Derive transcript path from the video output path.
        let transcript_path = TranscriptWriter::transcript_path(&video_output_path);

        // Spawn the engine event subscriber task (tasks 7.1–7.5).
        let handle = tokio::spawn(async move {
            // Create the transcript file.
            let mut writer = match TranscriptWriter::new(&transcript_path) {
                Ok(w) => Some(w),
                Err(e) => {
                    error!("[Transcription] Failed to create transcript file {:?}: {}", transcript_path, e);
                    None
                }
            };

            loop {
                match engine_rx.recv().await {
                    Ok(EngineEvent::TranscriptionSegment(seg)) => {
                        tracing::debug!(
                            "[Transcription] Segment: offset={}ms, text={}",
                            seg.timestamp_offset_ms, seg.text
                        );

                        // Convert vtx-common segment → OmniRec IPC segment and store for polling.
                        let ipc_seg = TranscriptionSegment::from_vtx(&seg);
                        if let Ok(mut segs) = segments_storage.lock() {
                            segs.push(ipc_seg.clone());
                        }

                        // Broadcast as a service event for subscribed CLI clients.
                        let _ = event_tx.send(ServiceEvent::TranscriptionSegment {
                            timestamp_secs: ipc_seg.timestamp_secs,
                            text: ipc_seg.text.clone(),
                        });

                        // Append to the transcript file.
                        if let Some(ref mut w) = writer {
                            w.append_segment(seg.timestamp_offset_ms, &seg.text);
                        }
                    }
                    Ok(EngineEvent::RecordingStopped { duration_ms }) => {
                        info!("[Transcription] Engine recording stopped ({}ms)", duration_ms);
                        // Do NOT break here — wait for TranscriptionSegment events which may
                        // arrive after RecordingStopped (transcription is async).
                    }
                    Ok(EngineEvent::CaptureStateChanged { capturing: false, .. }) => {
                        // Capture has fully stopped. All pending transcription segments have
                        // been emitted before this event.
                        tracing::debug!("[Transcription] Capture stopped, exiting event loop");
                        break;
                    }
                    Ok(_) => {
                        // Other events (RecordingStarted, SpeechStarted, etc.) — no action needed.
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("[Transcription] Event receiver lagged: dropped {} events", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::debug!("[Transcription] Engine broadcast channel closed");
                        break;
                    }
                }
            }

            info!("[Transcription] Event subscriber task finished");
        });

        let mut task = self.engine_task.lock().await;
        *task = Some(handle);
    }

    /// Start audio capture from up to two sources with optional AEC.
    async fn start_audio_capture_dual(
        &self,
        system_source_id: Option<&str>,
        mic_source_id: Option<&str>,
        _aec_enabled: bool,
    ) -> Result<(AudioReceiver, StopHandle), String> {
        if let Some(source_id) = system_source_id {
            let backend = crate::capture::get_backend();
            backend
                .start_audio_capture(source_id)
                .map_err(|e| e.to_string())
        } else if let Some(source_id) = mic_source_id {
            let backend = crate::capture::get_backend();
            backend
                .start_audio_capture(source_id)
                .map_err(|e| e.to_string())
        } else {
            Err("No audio source specified".to_string())
        }
    }

    /// Start broadcasting elapsed time updates.
    async fn start_elapsed_broadcast(&self) {
        let event_tx = self.event_tx.clone();
        let recording_start = *self.recording_start.lock().await;
        let stop_flag = self.stop_flag.lock().await.clone();

        if let (Some(start), Some(flag)) = (recording_start, stop_flag) {
            let handle = tokio::spawn(async move {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    if flag.load(Ordering::Relaxed) {
                        break;
                    }
                    let elapsed = start.elapsed().as_secs();
                    let _ = event_tx.send(ServiceEvent::ElapsedTime(elapsed));
                }
            });

            let mut task = self.elapsed_task.lock().await;
            *task = Some(handle);
        }
    }

    /// Stop the current recording and save the file.
    pub async fn stop_recording(&self) -> Result<RecordingResult, String> {
        {
            let state = self.state.read().await;
            if *state != RecordingState::Recording {
                return Err("Not currently recording".to_string());
            }
        }

        let format = self.get_output_format().await;

        self.set_state(RecordingState::Saving).await;

        // Stop elapsed time broadcasting
        {
            let mut task = self.elapsed_task.lock().await;
            if let Some(handle) = task.take() {
                handle.abort();
            }
        }

        // Signal stop flags (video and FFmpeg audio)
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

        // Stop vtx-engine recording — this submits the accumulated audio buffer
        // for transcription. TranscriptionSegment events will follow asynchronously.
        if self.engine.is_recording() {
            self.engine.stop_recording();
            info!("[Transcription] Engine stop_recording() called");
        }

        // Wait for video/audio encoding to complete
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

        // Transcode if needed
        let file_path = if format != OutputFormat::Mp4 {
            self.broadcast(ServiceEvent::TranscodingStarted {
                format: format!("{:?}", format),
            });

            match crate::encoder::transcode_video(&source_path, format) {
                Ok(transcoded_path) => {
                    self.broadcast(ServiceEvent::TranscodingComplete {
                        success: true,
                        path: Some(transcoded_path.display().to_string()),
                    });
                    transcoded_path
                }
                Err(e) => {
                    error!("Transcoding failed: {}", e);
                    self.broadcast(ServiceEvent::TranscodingComplete {
                        success: false,
                        path: None,
                    });
                    source_path.clone()
                }
            }
        } else {
            source_path.clone()
        };

        // Clean up and reset to idle
        self.cleanup().await;

        info!("Recording saved: {}", file_path.display());

        Ok(RecordingResult {
            source_path,
            file_path,
        })
    }

    /// Clean up internal state and reset to idle.
    async fn cleanup(&self) {
        // Stop vtx-engine capture. The audio loop exit triggers a CaptureStateChanged event,
        // which causes the event subscriber task to finish.
        if self.engine.is_capturing() {
            if let Err(e) = self.engine.stop_capture().await {
                warn!("[Transcription] Engine stop_capture error: {}", e);
            }
        }

        // Wait for the engine event subscriber task to finish (ensures transcript is fully written).
        {
            let mut task = self.engine_task.lock().await;
            if let Some(handle) = task.take() {
                let _ = handle.await;
            }
        }

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
        self.set_state(RecordingState::Idle).await;
    }

    /// Broadcast shutdown event to all subscribers.
    pub fn shutdown(&self) {
        if self.engine.is_recording() {
            self.engine.stop_recording();
        }
        self.engine.shutdown();
        self.broadcast(ServiceEvent::Shutdown);
    }
}

impl Default for RecordingManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global singleton for the recording manager
use std::sync::OnceLock;
static RECORDING_MANAGER: OnceLock<RecordingManager> = OnceLock::new();

/// Get the global recording manager instance.
pub fn get_recording_manager() -> &'static RecordingManager {
    RECORDING_MANAGER.get_or_init(RecordingManager::new)
}
