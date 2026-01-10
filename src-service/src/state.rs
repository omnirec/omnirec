//! Recording state management for the OmniRec service.
//!
//! This module manages the recording lifecycle, including:
//! - Recording state (idle, recording, saving)
//! - Output format configuration
//! - Audio configuration
//! - Transcription configuration
//! - Elapsed time tracking
//! - Event broadcasting to subscribed clients

use crate::capture::{
    AudioCaptureBackend, AudioReceiver, CaptureBackend, CaptureRegion, FrameReceiver, StopHandle,
};
use crate::encoder::{
    encode_frames, encode_frames_with_audio, encode_frames_with_audio_and_transcription,
    AudioEncoderConfig,
};
use crate::transcription::TranscribeState;
use omnirec_common::{AudioConfig, OutputFormat, RecordingState, TranscriptionConfig};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tracing::{error, info, warn};

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
    /// Service is shutting down
    Shutdown,
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
    /// Transcription configuration
    transcription_config: RwLock<TranscriptionConfig>,
    /// Transcription state (active during recording)
    transcription_state: Mutex<TranscribeState>,
    /// Transcription processing task handle
    transcription_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Broadcast channel for events
    event_tx: broadcast::Sender<ServiceEvent>,
    /// Elapsed time update task handle
    elapsed_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl RecordingManager {
    /// Create a new recording manager.
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            state: RwLock::new(RecordingState::Idle),
            stop_flag: Mutex::new(None),
            audio_stop_flag: Mutex::new(None),
            recording_start: Mutex::new(None),
            encoding_task: Mutex::new(None),
            output_format: RwLock::new(OutputFormat::default()),
            audio_config: RwLock::new(AudioConfig::default()),
            transcription_config: RwLock::new(TranscriptionConfig::default()),
            transcription_state: Mutex::new(TranscribeState::new()),
            transcription_task: Mutex::new(None),
            event_tx,
            elapsed_task: Mutex::new(None),
        }
    }

    /// Subscribe to service events.
    #[allow(dead_code)] // Will be used when event streaming is implemented
    pub fn subscribe(&self) -> broadcast::Receiver<ServiceEvent> {
        self.event_tx.subscribe()
    }

    /// Broadcast an event to all subscribers.
    fn broadcast(&self, event: ServiceEvent) {
        // Ignore send errors (no subscribers)
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

    /// Get current transcription status.
    pub async fn get_transcription_status(&self) -> omnirec_common::TranscriptionStatus {
        let state = self.transcription_state.lock().await;
        let active = state.is_active();
        let queue = state.queue();

        omnirec_common::TranscriptionStatus {
            model_loaded: false, // TODO: Check if model is loaded
            active,
            segments_processed: queue.segments_processed() as u32,
            queue_depth: queue.queue_depth() as u32,
            error: None,
        }
    }

    /// Get a clone of the stop flag (for external stop monitoring).
    #[allow(dead_code)] // Will be used for external stop monitoring
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
        // Get audio config
        let audio_cfg = self.get_audio_config().await;
        let has_system_audio = audio_cfg.enabled && audio_cfg.source_id.is_some();
        let has_microphone = audio_cfg.enabled && audio_cfg.microphone_id.is_some();
        let audio_enabled = has_system_audio || has_microphone;

        // Get transcription config
        let transcription_cfg = self.get_transcription_config().await;
        // Transcription requires system audio (can't transcribe mic-only reliably)
        let transcription_enabled = transcription_cfg.enabled && has_system_audio;

        eprintln!(
            "[Recording] Audio config: enabled={}, source_id={:?}, mic_id={:?}, has_system_audio={}, has_mic={}, audio_enabled={}",
            audio_cfg.enabled, audio_cfg.source_id, audio_cfg.microphone_id, has_system_audio, has_microphone, audio_enabled
        );
        eprintln!(
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
                    // Store audio stop flag
                    {
                        let mut flag = self.audio_stop_flag.lock().await;
                        *flag = Some(audio_stop);
                    }

                    let audio_encoder_config = AudioEncoderConfig::default();

                    if transcription_enabled {
                        eprintln!("[Transcription] Transcription is ENABLED - creating channel and starting task");

                        // Generate output path upfront so transcription can use the same base name
                        let video_output_path = match crate::encoder::generate_output_path() {
                            Ok(path) => path,
                            Err(e) => {
                                error!("Failed to generate output path: {}", e);
                                return Err(e);
                            }
                        };
                        eprintln!("[Transcription] Video output path: {:?}", video_output_path);

                        // Get model path from config
                        let model_path = transcription_cfg.model_path.as_ref().map(PathBuf::from);
                        eprintln!("[Transcription] Model path from config: {:?}", model_path);

                        // Create transcription channel
                        let (transcription_tx, transcription_rx) = mpsc::channel::<Vec<f32>>(256);

                        // Start transcription processing task with video output path
                        // Note: Transcription uses channel disconnection to detect stop, not the stop_flag,
                        // because the video capture stop_flag can be set during PipeWire format renegotiation.
                        self.start_transcription_task(
                            transcription_rx,
                            video_output_path.clone(),
                            model_path,
                        )
                        .await;

                        // Use encoder with transcription support, passing the pre-generated output path
                        tokio::spawn(encode_frames_with_audio_and_transcription(
                            frame_rx,
                            audio_rx,
                            stop_flag.clone(),
                            audio_encoder_config,
                            Some(transcription_tx),
                            Some(video_output_path),
                        ))
                    } else {
                        eprintln!("[Transcription] Transcription is DISABLED (config.enabled={}, has_system_audio={})", 
                            transcription_cfg.enabled, has_system_audio);
                        // Standard audio encoding without transcription
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

    /// Start the transcription processing task.
    ///
    /// This task receives audio samples from the encoder and feeds them to the
    /// transcription state machine for voice detection and transcription.
    ///
    /// # Arguments
    /// * `transcription_rx` - Channel to receive audio samples from the encoder
    /// * `video_output_path` - Path to the video output file (transcript will be derived from this)
    /// * `model_path` - Optional custom model path (uses default if None)
    async fn start_transcription_task(
        &self,
        transcription_rx: mpsc::Receiver<Vec<f32>>,
        video_output_path: PathBuf,
        model_path: Option<PathBuf>,
    ) {
        // Initialize transcription state with the video output path
        {
            let mut state = self.transcription_state.lock().await;
            if let Err(e) =
                state.start_with_model(video_output_path.clone(), 48000, 2, model_path.clone())
            {
                error!("Failed to start transcription: {}", e);
                return;
            }
        }

        // Get queue reference for final stats
        let transcription_state = self.transcription_state.lock().await;
        let queue = transcription_state.queue();
        drop(transcription_state);

        // Use a std channel for the blocking thread
        let (std_tx, std_rx) = std::sync::mpsc::channel::<Vec<f32>>();

        // Spawn a blocking thread for transcription processing
        // TranscribeState.process_samples() is synchronous
        let queue_clone = queue.clone();
        let video_path_for_thread = video_output_path.clone();
        let model_path_for_thread = model_path.clone();
        let transcription_thread = std::thread::spawn(move || {
            eprintln!("[Transcription] Worker thread started");
            let mut transcribe_state = TranscribeState::new();

            // Start the transcription state with the video output path and model path
            if let Err(e) = transcribe_state.start_with_model(
                video_path_for_thread,
                48000,
                2,
                model_path_for_thread,
            ) {
                eprintln!(
                    "[Transcription] Failed to start transcription in thread: {}",
                    e
                );
                return;
            }
            eprintln!("[Transcription] TranscribeState initialized in worker thread");

            let mut total_samples_received: u64 = 0;
            let mut last_stats_time = std::time::Instant::now();

            // Note: We don't check stop_flag here because it can be set by PipeWire during
            // format renegotiation, which is normal behavior. Instead, we rely on the channel
            // being disconnected when the encoder finishes (which happens when recording stops).
            loop {
                // Receive with timeout to allow periodic logging
                match std_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(samples) => {
                        total_samples_received += samples.len() as u64;
                        transcribe_state.process_samples(&samples);

                        // Log stats every 5 seconds
                        if last_stats_time.elapsed().as_secs() >= 5 {
                            let duration_secs = total_samples_received as f64 / 48000.0 / 2.0; // stereo
                            eprintln!(
                                "[Transcription] Received {:.1}s of audio ({} samples), queue depth: {}",
                                duration_secs,
                                total_samples_received,
                                queue_clone.queue_depth()
                            );
                            last_stats_time = std::time::Instant::now();
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        continue;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        eprintln!("[Transcription] Channel disconnected, exiting loop");
                        break;
                    }
                }
            }

            // Finalize transcription
            transcribe_state.stop();
            eprintln!(
                "[Transcription] Thread finished - total {:.1}s of audio, {} segments processed",
                total_samples_received as f64 / 48000.0 / 2.0,
                queue_clone.segments_processed()
            );
        });

        // Spawn async task to forward samples from async channel to std channel
        // Note: We don't check stop_flag here because it can be set by PipeWire during
        // format renegotiation. The encoder will close the transcription_tx channel when
        // recording actually ends, which will cause recv() to return None.
        let mut transcription_rx = transcription_rx;
        let handle = tokio::spawn(async move {
            let mut forwarded_count: u64 = 0;

            loop {
                match transcription_rx.recv().await {
                    Some(samples) => {
                        forwarded_count += 1;
                        // Forward to the blocking thread
                        if std_tx.send(samples).is_err() {
                            // Thread has exited
                            tracing::warn!("[Transcription] Forwarder: worker thread exited");
                            break;
                        }
                    }
                    None => {
                        // Channel closed - encoder has finished
                        eprintln!(
                            "[Transcription] Forwarder: channel closed after {} batches",
                            forwarded_count
                        );
                        break;
                    }
                }
            }

            // Drop sender to signal thread to stop
            drop(std_tx);

            // Wait for thread to finish
            let _ = transcription_thread.join();

            info!(
                "[Transcription] Task finished ({} segments processed)",
                queue.segments_processed()
            );
        });

        // Store the task handle
        {
            let mut task = self.transcription_task.lock().await;
            *task = Some(handle);
        }
    }

    /// Start audio capture from up to two sources with optional AEC.
    async fn start_audio_capture_dual(
        &self,
        system_source_id: Option<&str>,
        mic_source_id: Option<&str>,
        _aec_enabled: bool,
    ) -> Result<(AudioReceiver, StopHandle), String> {
        // TODO: Implement dual audio capture with AEC
        // For now, just capture system audio if available
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
        self.set_state(RecordingState::Saving).await;

        // Stop elapsed time broadcasting
        {
            let mut task = self.elapsed_task.lock().await;
            if let Some(handle) = task.take() {
                handle.abort();
            }
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
                    // Return source path on transcode failure
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
        // Stop transcription if active
        {
            let mut state = self.transcription_state.lock().await;
            if state.is_active() {
                state.stop();
            }
        }
        // Wait for transcription task to complete
        {
            let mut task = self.transcription_task.lock().await;
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
    #[allow(dead_code)] // Will be used for graceful shutdown
    pub fn shutdown(&self) {
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
