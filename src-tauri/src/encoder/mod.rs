//! Video encoding module using FFmpeg via ffmpeg-sidecar.
//!
//! On Windows and macOS, FFmpeg is bundled as a Tauri sidecar binary alongside
//! the application. On Linux, the system-installed FFmpeg is used instead.
//!
//! When audio is enabled, audio samples are streamed to FFmpeg via a named pipe
//! as a second input alongside the video frames on stdin. FFmpeg muxes both
//! streams into a single MP4 file in real-time -- no post-recording mux step.

pub mod audio_pipe;

use crate::capture::types::CapturedFrame;
use audio_pipe::{f32_mono_to_s16le, AudioPipe};
use chrono::Local;
use directories::UserDirs;
use ffmpeg_sidecar::command::FfmpegCommand;
use std::io::Write;
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use vtx_engine::EngineEvent;

/// Resolve the path to the FFmpeg binary.
///
/// On Windows and macOS, FFmpeg is bundled as a Tauri sidecar binary
/// (configured in `tauri.conf.json` under `externalBin`). The sidecar is
/// placed adjacent to the application executable with a platform-triple suffix
/// that Tauri strips at install time, making it available as `ffmpeg` (or
/// `ffmpeg.exe`) next to the main binary.
///
/// On Linux, FFmpeg is declared as a system package dependency in the
/// deb/rpm/AUR packaging, so we use the system-installed binary from PATH.
fn resolve_ffmpeg_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        // Linux: use system FFmpeg from PATH
        PathBuf::from("ffmpeg")
    }
    #[cfg(not(target_os = "linux"))]
    {
        // Windows/macOS: use the bundled sidecar binary adjacent to our executable.
        // ffmpeg-sidecar's built-in path resolution does exactly this: it looks
        // for an "ffmpeg" (or "ffmpeg.exe") binary next to current_exe(), which
        // is where Tauri places externalBin sidecars after installation.
        ffmpeg_sidecar::paths::ffmpeg_path()
    }
}

/// Create a new FfmpegCommand using the resolved FFmpeg path.
fn new_ffmpeg_command() -> FfmpegCommand {
    FfmpegCommand::new_with_path(resolve_ffmpeg_path())
}

/// Detect the best available H.264 encoder.
/// Returns the encoder name to use with FFmpeg.
fn detect_h264_encoder() -> &'static str {
    // Check which encoders are available by running ffmpeg -encoders
    let output = Command::new(resolve_ffmpeg_path())
        .args(["-encoders", "-hide_banner"])
        .output();

    let encoders_output = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => {
            tracing::debug!("[Encoder] Failed to run ffmpeg -encoders: {}", e);
            String::new()
        }
    };

    tracing::debug!("[Encoder] Checking available H.264 encoders...");

    // Preference order: libx264 (best quality/compat), then hardware encoders, then fallback
    // Note: Fedora's ffmpeg-free doesn't include libx264, so we check hardware encoders too
    let encoder_preferences = [
        ("libx264", "libx264"), // Software, best compatibility (not on Fedora ffmpeg-free)
        ("libopenh264", "libopenh264"), // OpenH264 (Cisco, available on Fedora)
        ("h264_vaapi", "h264_vaapi"), // VAAPI (AMD/Intel) - common on Linux
        ("h264_nvenc", "h264_nvenc"), // NVIDIA
        ("h264_amf", "h264_amf"), // AMD AMF
        ("h264_qsv", "h264_qsv"), // Intel QuickSync
        ("h264_v4l2m2m", "h264_v4l2m2m"), // V4L2 (RPi, etc.)
        ("h264_vulkan", "h264_vulkan"), // Vulkan
    ];

    for (search_name, encoder_name) in encoder_preferences {
        // Check if the encoder is listed (search for the encoder name followed by space or end)
        if encoders_output.contains(&format!(" {} ", search_name))
            || encoders_output.contains(&format!(" {}\n", search_name))
            || encoders_output.lines().any(|l| l.contains(search_name))
        {
            tracing::debug!("[Encoder] Found H.264 encoder: {}", encoder_name);
            return encoder_name;
        }
    }

    // Last resort fallback - try libx264 anyway
    tracing::warn!("[Encoder] Warning: No H.264 encoder detected in ffmpeg output!");
    tracing::debug!(
        "[Encoder] Available encoders: {}",
        encoders_output
            .lines()
            .filter(|l| l.contains("264") || l.contains("h264"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    tracing::debug!("[Encoder] Trying libx264 as fallback (may fail on Fedora without RPM Fusion)");
    "libx264"
}

/// Video encoder that receives frames and encodes to MP4.
/// When an audio pipe path is provided, FFmpeg is started with dual inputs
/// (rawvideo on stdin + raw PCM audio on the named pipe) for real-time muxing.
pub struct VideoEncoder {
    video_stdin: Option<ChildStdin>,
    child: Option<std::process::Child>,
    output_path: PathBuf,
    width: u32,
    height: u32,
}

impl VideoEncoder {
    /// Create a new encoder with the given frame dimensions and optional output path.
    pub fn new_with_options(
        width: u32,
        height: u32,
        output_path: Option<PathBuf>,
    ) -> Result<Self, String> {
        let output_path = match output_path {
            Some(p) => p,
            None => generate_output_path()?,
        };

        // Ensure dimensions are even (required by many codecs including h264)
        let width = width & !1;
        let height = height & !1;

        if width == 0 || height == 0 {
            return Err(format!("Invalid dimensions: {}x{}", width, height));
        }

        Ok(Self {
            video_stdin: None,
            child: None,
            output_path,
            width,
            height,
        })
    }

    /// Start the FFmpeg encoding process.
    ///
    /// When `audio_pipe_path` is provided, FFmpeg is started with two inputs:
    /// - Input 0: rawvideo from stdin (video frames)
    /// - Input 1: raw s16le PCM from the named pipe (audio)
    ///
    /// The audio is encoded to AAC and muxed into the MP4 in real-time.
    pub fn start(&mut self, audio_pipe_path: Option<&str>) -> Result<(), String> {
        // Detect available H.264 encoder
        let encoder = detect_h264_encoder();

        // Build the FFmpeg command using std::process for better stdin control
        let mut command = new_ffmpeg_command();
        command
            // Input 0: raw video frames from stdin
            .args(["-f", "rawvideo"])
            .args(["-pix_fmt", "bgra"])
            .args(["-s", &format!("{}x{}", self.width, self.height)])
            .args(["-r", "30"]) // 30 FPS
            .args(["-i", "-"]); // Read from stdin

        // Input 1: raw PCM audio from named pipe (if audio enabled)
        if let Some(pipe_path) = audio_pipe_path {
            command
                .args(["-f", "s16le"])
                .args(["-ar", "48000"])
                .args(["-ac", "1"])
                .args(["-i", pipe_path]);
        }

        // Output: H.264 in MP4 container
        command.args(["-c:v", encoder]);

        // Add encoder-specific options
        match encoder {
            "libx264" => {
                command
                    .args(["-preset", "ultrafast"]) // Fast encoding for real-time
                    .args(["-crf", "23"]); // Good quality/size balance
            }
            "libopenh264" => {
                // OpenH264 has limited options
                command.args(["-b:v", "2M"]); // Target bitrate
            }
            "h264_vaapi" => {
                // VAAPI needs different options
                command.args(["-qp", "23"]); // Quality parameter
            }
            "h264_nvenc" | "h264_amf" => {
                command
                    .args(["-preset", "p1"]) // Fastest preset
                    .args(["-rc", "vbr"])
                    .args(["-cq", "23"]);
            }
            _ => {
                // Generic options for other encoders
                tracing::debug!("[Encoder] Using generic options for encoder: {}", encoder);
            }
        }

        command.args(["-pix_fmt", "yuv420p"]); // Compatible pixel format

        // Audio encoding (if audio pipe provided)
        if audio_pipe_path.is_some() {
            command
                .args(["-c:a", "aac"])
                .args(["-b:a", "192k"])
                .args(["-map", "0:v"]) // Video from input 0 (stdin)
                .args(["-map", "1:a"]) // Audio from input 1 (pipe)
                .args(["-shortest"]); // Match durations
        }

        // Use fragmented MP4 so the file is written progressively to disk.
        // Without this, FFmpeg buffers the entire file in memory and writes
        // the moov atom at the end -- the file doesn't appear on disk until
        // FFmpeg exits, and a crash/kill loses all data.
        //
        // frag_keyframe: start a new fragment at each keyframe
        // empty_moov:    write an empty moov at start (no buffering needed)
        command.args(["-movflags", "frag_keyframe+empty_moov"]);

        command
            .args(["-y"]) // Overwrite output
            .arg(self.output_path.to_string_lossy().to_string());

        // Get the inner command and configure stdin
        let inner_command = command.as_inner_mut();
        inner_command.stdin(Stdio::piped());
        inner_command.stdout(Stdio::null());
        inner_command.stderr(Stdio::piped());

        let mut child = inner_command
            .spawn()
            .map_err(|e| format!("Failed to start FFmpeg: {}", e))?;

        let stdin = child.stdin.take().ok_or("Failed to get FFmpeg stdin")?;

        // Spawn a thread to read stderr and log FFmpeg errors
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    tracing::debug!("[FFmpeg] {}", line);
                }
                tracing::debug!("[FFmpeg] stderr reader thread exiting");
            });
        }

        self.video_stdin = Some(stdin);
        self.child = Some(child);

        Ok(())
    }

    /// Write a frame to the encoder.
    pub fn write_frame(&mut self, frame: &CapturedFrame) -> Result<(), String> {
        // Handle frames that may be slightly larger than encoder dimensions
        // (can happen due to even-dimension rounding)
        if frame.width < self.width || frame.height < self.height {
            tracing::debug!(
                "Skipping frame: dimensions {}x{} smaller than encoder {}x{}",
                frame.width,
                frame.height,
                self.width,
                self.height
            );
            return Ok(());
        }

        if let Some(ref mut stdin) = self.video_stdin {
            // If frame is larger than encoder dimensions, crop it
            if frame.width == self.width && frame.height == self.height {
                // Exact match, write directly
                stdin
                    .write_all(&frame.data)
                    .map_err(|e| format!("Failed to write frame: {}", e))?;
            } else {
                // Need to crop - extract only the rows/columns we need
                let src_row_bytes = (frame.width * 4) as usize;
                let dst_row_bytes = (self.width * 4) as usize;

                for y in 0..self.height as usize {
                    let src_start = y * src_row_bytes;
                    let src_end = src_start + dst_row_bytes;
                    if src_end <= frame.data.len() {
                        stdin
                            .write_all(&frame.data[src_start..src_end])
                            .map_err(|e| format!("Failed to write frame row: {}", e))?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Get the output path.
    pub fn output_path(&self) -> &PathBuf {
        &self.output_path
    }

    /// Finalize the encoding and close the output file.
    pub fn finish(mut self) -> Result<PathBuf, String> {
        // Close stdin to signal end of input
        drop(self.video_stdin.take());

        // Wait for FFmpeg to finish
        if let Some(mut child) = self.child.take() {
            // Read stderr for error messages
            let stderr_output = if let Some(mut stderr) = child.stderr.take() {
                use std::io::Read;
                let mut output = String::new();
                let _ = stderr.read_to_string(&mut output);
                output
            } else {
                String::new()
            };

            let status = child
                .wait()
                .map_err(|e| format!("FFmpeg process error: {}", e))?;

            if !status.success() {
                let error_msg = if stderr_output.is_empty() {
                    format!("FFmpeg encoding failed with exit code: {:?}", status.code())
                } else {
                    format!(
                        "FFmpeg failed: {}",
                        stderr_output.lines().last().unwrap_or(&stderr_output)
                    )
                };
                return Err(error_msg);
            }
        }

        Ok(self.output_path)
    }
}

/// Generate a unique output filename in the default output directory (Videos folder).
pub fn generate_output_path() -> Result<PathBuf, String> {
    // Use system Videos directory or fallback to temp
    let output_dir = get_default_output_dir()?;

    // Ensure directory exists
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir)
            .map_err(|e| format!("Failed to create output directory: {}", e))?;
    }

    let timestamp = Local::now().format("%Y-%m-%d_%H%M%S");
    let filename = format!("recording_{}.mp4", timestamp);

    tracing::debug!("[Encoder] Output path: {:?}", output_dir.join(&filename));
    Ok(output_dir.join(filename))
}

/// Get the default output directory (system Videos folder or temp fallback).
fn get_default_output_dir() -> Result<PathBuf, String> {
    let user_dirs = UserDirs::new().ok_or("Could not determine user directories")?;

    // Try Videos directory first, fall back to home directory
    let output_dir = user_dirs
        .video_dir()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| {
            let home = user_dirs.home_dir().to_path_buf();
            let videos = home.join("Videos");
            // Try to create Videos directory if it doesn't exist
            if !videos.exists() && std::fs::create_dir_all(&videos).is_ok() {
                return videos;
            }
            // Fall back to home directory if Videos exists or creation failed
            if videos.exists() {
                videos
            } else {
                home
            }
        });

    Ok(output_dir)
}

/// Target frame rate for output video
const TARGET_FPS: u32 = 30;
/// Frame interval in milliseconds
const FRAME_INTERVAL_MS: u64 = 1000 / TARGET_FPS as u64;

/// Unified encoding function that receives video frames and optionally muxes
/// audio from vtx-engine's `RawAudioData` events in real-time.
///
/// This is a **blocking** function that runs on a dedicated thread (via
/// `tokio::task::spawn_blocking`). All I/O -- writing video frames to FFmpeg's
/// stdin, writing audio to the named pipe, and waiting for FFmpeg to exit --
/// happens on this thread, keeping the tokio async runtime free for event
/// processing, transcription, and UI updates.
///
/// When `audio_rx` is `Some`, a named pipe is created for audio and FFmpeg is
/// started with dual inputs. Audio samples from `RawAudioData` events are
/// converted to s16le PCM and written to the pipe. When `audio_rx` is `None`,
/// the encoder produces a video-only MP4.
///
/// # Arguments
/// * `frame_rx` - Channel to receive video frames from capture
/// * `audio_rx` - Optional vtx-engine broadcast receiver for `RawAudioData` events
/// * `stop_flag` - Flag to signal recording should stop
/// * `output_path` - Optional pre-generated output path (generated if None)
pub fn encode_frames(
    mut frame_rx: mpsc::Receiver<CapturedFrame>,
    mut audio_rx: Option<broadcast::Receiver<EngineEvent>>,
    stop_flag: Arc<AtomicBool>,
    output_path: Option<PathBuf>,
) -> Result<PathBuf, String> {
    let has_audio = audio_rx.is_some();
    tracing::debug!(
        "[Encoder] encode_frames task started (audio: {}), waiting for first frame...",
        has_audio
    );

    // Wait for first frame to get dimensions (blocking)
    let first_frame = frame_rx.blocking_recv().ok_or_else(|| {
        tracing::debug!("[Encoder] recv() returned None - channel closed without frames");
        "No frames received".to_string()
    })?;

    // Record the exact moment we received the first video frame.
    // This is our reference point for A/V sync.
    let video_start_time = std::time::Instant::now();

    tracing::debug!(
        "[Encoder] Got first frame: {}x{}",
        first_frame.width,
        first_frame.height
    );

    // Set up audio pipe if audio is enabled.
    // The pipe must be created BEFORE FFmpeg is spawned because FFmpeg needs
    // the pipe path as a command-line argument. The pipe is opened for writing
    // AFTER FFmpeg starts (FFmpeg blocks waiting for the writer).
    let mut audio_pipe = if has_audio {
        Some(AudioPipe::create()?)
    } else {
        None
    };

    // Create and start the video encoder
    let mut encoder =
        VideoEncoder::new_with_options(first_frame.width, first_frame.height, output_path)?;
    let pipe_path = audio_pipe.as_ref().map(|p| p.path().to_string());
    encoder.start(pipe_path.as_deref())?;

    tracing::debug!("[Encoder] Video output path: {:?}", encoder.output_path());

    // Write the first video frame BEFORE opening the audio pipe.
    // FFmpeg processes inputs sequentially: it reads from stdin (input 0) first,
    // then opens the named pipe (input 1). If we try to open the pipe before
    // sending any video data, we deadlock -- pipe.open() blocks waiting for
    // FFmpeg to connect, while FFmpeg blocks waiting for video data on stdin.
    encoder.write_frame(&first_frame)?;

    // Open the audio pipe for writing (unblocks FFmpeg).
    // This must happen AFTER FFmpeg starts AND after the first video frame is
    // written, so FFmpeg has moved past stdin initialization and is ready to
    // open the audio pipe input. This blocks until FFmpeg connects.
    if let Some(ref mut pipe) = audio_pipe {
        tracing::debug!("[Encoder] Opening audio pipe for writing: {}", pipe.path());
        pipe.open()?;
        tracing::debug!("[Encoder] Audio pipe opened");
    }

    tracing::debug!("[Encoder] Encoder initialized, entering main loop...");

    // Spawn a dedicated audio writer thread when audio is enabled.
    // Audio and video MUST be written from separate threads to prevent a
    // deadlock: FFmpeg's MP4 muxer synchronizes both inputs, so if video
    // gets ahead of audio, FFmpeg stops reading from stdin until audio
    // catches up. If audio can't be written because we're stuck writing
    // video on the same thread, we deadlock.
    let audio_thread = if let Some(mut rx) = audio_rx.take() {
        let mut pipe = audio_pipe
            .take()
            .expect("audio_pipe must exist when audio_rx is Some");
        let audio_stop = stop_flag.clone();
        let audio_video_start = video_start_time;
        Some(std::thread::spawn(move || -> Result<u64, String> {
            let mut samples_written = 0u64;
            let mut first_sample_offset: Option<u64> = None;
            let mut expected_next_offset: u64 = 0;

            loop {
                if audio_stop.load(Ordering::Relaxed) {
                    // Drain any remaining events before exiting
                    loop {
                        match rx.try_recv() {
                            Ok(EngineEvent::RawAudioData(data)) => {
                                let pcm_data = f32_mono_to_s16le(&data.samples);
                                if let Err(e) = pipe.write_all(&pcm_data) {
                                    tracing::debug!(
                                        "[AudioWriter] Pipe write error during drain: {}",
                                        e
                                    );
                                    break;
                                }
                                samples_written += data.samples.len() as u64;
                            }
                            Ok(_) => {}
                            _ => break,
                        }
                    }
                    tracing::debug!("[AudioWriter] Stop flag set, exiting");
                    break;
                }

                match rx.try_recv() {
                    Ok(EngineEvent::RawAudioData(data)) => {
                        // First audio: pad silence for the gap between video start and audio start
                        if first_sample_offset.is_none() {
                            first_sample_offset = Some(data.sample_offset);
                            expected_next_offset = data.sample_offset;

                            let delay = std::time::Instant::now().duration_since(audio_video_start);
                            let delay_ms = delay.as_millis() as u64;
                            tracing::debug!(
                                "[AudioWriter] First audio at offset={}, delay from video: {}ms",
                                data.sample_offset,
                                delay_ms
                            );

                            // Write silence for the startup gap
                            if delay_ms > 0 {
                                let silence_samples = (delay_ms * data.sample_rate as u64) / 1000;
                                pipe.write_silence(silence_samples as usize)?;
                            }
                        }

                        // Gap detection: fill missing samples with silence
                        if data.sample_offset > expected_next_offset {
                            let gap = data.sample_offset - expected_next_offset;
                            if gap > 0 {
                                tracing::debug!(
                                    "[AudioWriter] Audio gap detected: {} samples",
                                    gap
                                );
                                pipe.write_silence(gap as usize)?;
                            }
                        }

                        // Convert f32 mono to s16le and write to pipe
                        let pcm_data = f32_mono_to_s16le(&data.samples);
                        pipe.write_all(&pcm_data)?;

                        samples_written += data.samples.len() as u64;
                        expected_next_offset = data.sample_offset + data.samples.len() as u64;
                    }
                    Ok(_) => {
                        // Ignore other engine events
                    }
                    Err(broadcast::error::TryRecvError::Empty) => {
                        // No audio events yet, sleep briefly
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err(broadcast::error::TryRecvError::Lagged(n)) => {
                        tracing::warn!("[AudioWriter] Audio event receiver lagged: {} events", n);
                    }
                    Err(broadcast::error::TryRecvError::Closed) => {
                        tracing::debug!("[AudioWriter] Audio broadcast channel closed");
                        break;
                    }
                }
            }

            // Drop pipe to signal EOF to FFmpeg's audio input
            drop(pipe);
            tracing::debug!(
                "[AudioWriter] Thread exiting, wrote {} samples",
                samples_written
            );
            Ok(samples_written)
        }))
    } else {
        None
    };

    let mut frames_written = 1u64;
    let mut last_frame = first_frame;
    let mut next_frame_time =
        video_start_time + std::time::Duration::from_millis(FRAME_INTERVAL_MS);

    let mut consecutive_empty_polls = 0u32;
    const MAX_EMPTY_POLLS: u32 = 100; // ~1 second at 10ms per poll

    loop {
        let now = std::time::Instant::now();

        // Check stop flag
        if stop_flag.load(Ordering::Relaxed) {
            tracing::debug!("[Encoder] Stop flag set, exiting loop");
            break;
        }

        // Process video frames (non-blocking)
        match frame_rx.try_recv() {
            Ok(frame) => {
                last_frame = frame;
                consecutive_empty_polls = 0;
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                consecutive_empty_polls += 1;
                if stop_flag.load(Ordering::Relaxed) && consecutive_empty_polls > 10 {
                    tracing::debug!("[Encoder] Stop flag set and no frames, exiting");
                    break;
                }
                if consecutive_empty_polls > MAX_EMPTY_POLLS {
                    if stop_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    consecutive_empty_polls = 0;
                }
            }
            Err(mpsc::error::TryRecvError::Disconnected) => {
                tracing::debug!("[Encoder] Video channel disconnected");
                break;
            }
        }

        // Write video frame at the target FPS rate.
        // Write at most one frame per iteration to keep the loop responsive
        // to the stop flag. If we fall behind, skip ahead rather than
        // trying to catch up with a burst of writes.
        if next_frame_time <= now {
            encoder.write_frame(&last_frame)?;
            frames_written += 1;
            next_frame_time += std::time::Duration::from_millis(FRAME_INTERVAL_MS);

            // If we're behind by more than one frame, skip ahead to real-time
            // rather than writing a burst of frames. A brief stutter in the
            // output is preferable to falling further behind.
            if next_frame_time <= now {
                let skipped =
                    (now.duration_since(next_frame_time).as_millis() as u64) / FRAME_INTERVAL_MS;
                if skipped > 0 {
                    next_frame_time +=
                        std::time::Duration::from_millis(skipped * FRAME_INTERVAL_MS);
                    tracing::debug!("[Encoder] Skipped {} frames to catch up", skipped);
                }
            }
        }

        // Sleep until next frame time (with some margin for processing)
        let sleep_duration = next_frame_time.saturating_duration_since(std::time::Instant::now());
        if !sleep_duration.is_zero() {
            std::thread::sleep(sleep_duration.min(std::time::Duration::from_millis(10)));
        }
    }

    // Wait for the audio writer thread to finish and close the pipe.
    // The pipe must be closed BEFORE finishing the encoder so FFmpeg sees
    // EOF on the audio input and can finalize the output file.
    let audio_samples_written = if let Some(thread) = audio_thread {
        match thread.join() {
            Ok(Ok(samples)) => samples,
            Ok(Err(e)) => {
                tracing::warn!("[Encoder] Audio writer thread error: {}", e);
                0
            }
            Err(_) => {
                tracing::warn!("[Encoder] Audio writer thread panicked");
                0
            }
        }
    } else {
        // No audio thread — drop the pipe if it exists (shouldn't, but defensive)
        drop(audio_pipe);
        0
    };

    let elapsed = video_start_time.elapsed().as_secs_f64();
    tracing::debug!(
        "[Encoder] Recording complete: {:.1}s, {} video frames, {} audio samples",
        elapsed,
        frames_written,
        audio_samples_written
    );

    // Finalize the encoder (waits for FFmpeg to write the MP4 trailer).
    encoder.finish()
}

/// Ensure FFmpeg is available. Should be called once at app startup.
///
/// On Windows and macOS, verifies that the bundled sidecar binary exists and is
/// executable. On Linux, falls back to runtime auto-download if the system
/// FFmpeg is not available (though it should be installed as a package dependency).
pub fn ensure_ffmpeg_blocking() -> Result<(), String> {
    let ffmpeg = resolve_ffmpeg_path();
    tracing::debug!("[FFmpeg] Resolved path: {}", ffmpeg.display());

    // Verify the binary is accessible by running `ffmpeg -version`
    match Command::new(&ffmpeg)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {
            tracing::debug!("[FFmpeg] Binary verified OK");
            Ok(())
        }
        Ok(status) => Err(format!(
            "FFmpeg binary at {} exited with status: {}",
            ffmpeg.display(),
            status
        )),
        Err(e) => {
            tracing::debug!("[FFmpeg] Binary not found at {}: {}", ffmpeg.display(), e);
            // On Linux, try auto-download as a last resort (system package may
            // not be installed in development environments)
            #[cfg(target_os = "linux")]
            {
                tracing::debug!("[FFmpeg] Attempting auto-download as fallback...");
                ffmpeg_sidecar::download::auto_download()
                    .map_err(|e| format!("FFmpeg not found and auto-download failed: {}", e))
            }
            #[cfg(not(target_os = "linux"))]
            {
                Err(format!(
                    "Bundled FFmpeg not found at {}. The application may not be installed correctly.",
                    ffmpeg.display()
                ))
            }
        }
    }
}

use omnirec_types::OutputFormat;
use std::path::Path;

/// Transcode a source MP4 file to the specified output format.
/// Returns the path to the transcoded file.
pub fn transcode_video(source_path: &Path, format: OutputFormat) -> Result<PathBuf, String> {
    // Generate output path with new extension
    let output_path = source_path.with_extension(format.extension());

    tracing::debug!(
        "[Transcode] Converting {} to {:?}",
        source_path.display(),
        format
    );
    tracing::debug!("[Transcode] Output: {}", output_path.display());

    let mut command = new_ffmpeg_command();

    // Input file
    command.args(["-i", source_path.to_string_lossy().as_ref()]);

    // Format-specific encoding settings
    match format {
        OutputFormat::Mp4 => {
            // No transcoding needed for MP4
            return Ok(source_path.to_path_buf());
        }
        OutputFormat::WebM => {
            // VP9 codec with good quality
            command.args(["-c:v", "libvpx-vp9"]);
            command.args(["-crf", "30"]);
            command.args(["-b:v", "0"]);
        }
        OutputFormat::Mkv => {
            // Remux only - copy video stream (very fast)
            command.args(["-c:v", "copy"]);
        }
        OutputFormat::QuickTime => {
            // Remux only - copy video stream (very fast)
            command.args(["-c:v", "copy"]);
            command.args(["-f", "mov"]);
        }
        OutputFormat::Gif => {
            // Generate palette for better quality GIF
            // Reduce fps to 15 for reasonable file size, keep original resolution
            command.args([
                "-vf",
                "fps=15,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse",
            ]);
        }
        OutputFormat::AnimatedPng => {
            // APNG format
            command.args(["-plays", "0"]); // Loop forever
            command.args(["-f", "apng"]);
        }
        OutputFormat::AnimatedWebp => {
            // WebP format with good quality
            command.args(["-c:v", "libwebp"]);
            command.args(["-lossless", "0"]);
            command.args(["-q:v", "75"]);
            command.args(["-loop", "0"]); // Loop forever
        }
    }

    // Overwrite output and set output path
    command.args(["-y"]);
    command.arg(output_path.to_string_lossy().to_string());

    // Get the inner command and configure for process output
    let inner_command = command.as_inner_mut();
    inner_command.stdout(Stdio::null());
    inner_command.stderr(Stdio::piped());

    let mut child = inner_command
        .spawn()
        .map_err(|e| format!("Failed to start FFmpeg for transcoding: {}", e))?;

    // Read stderr for progress/error messages
    let stderr_output = if let Some(mut stderr) = child.stderr.take() {
        use std::io::Read;
        let mut output = String::new();
        let _ = stderr.read_to_string(&mut output);
        output
    } else {
        String::new()
    };

    let status = child
        .wait()
        .map_err(|e| format!("FFmpeg transcoding process error: {}", e))?;

    if !status.success() {
        let error_msg = if stderr_output.is_empty() {
            format!(
                "FFmpeg transcoding failed with exit code: {:?}",
                status.code()
            )
        } else {
            format!(
                "FFmpeg transcoding failed: {}",
                stderr_output.lines().last().unwrap_or(&stderr_output)
            )
        };
        return Err(error_msg);
    }

    tracing::debug!("[Transcode] Successfully created {}", output_path.display());
    Ok(output_path)
}
