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
use std::time::{Duration, Instant, SystemTime};
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
    /// Encoder width (even-aligned).
    pub width: u32,
    /// Encoder height (even-aligned).
    pub height: u32,
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

        tracing::info!(
            "[Encoder] Starting FFmpeg with dimensions {}x{}, encoder: {}",
            self.width,
            self.height,
            encoder
        );

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
                .args(["-map", "1:a"]); // Audio from input 1 (pipe)
                                        // Note: -shortest removed - video duration should determine output length
        }

        command
            .args(["-y"]) // Overwrite output
            .arg(self.output_path.to_string_lossy().to_string());

        // Get the inner command and configure stdin
        let inner_command = command.as_inner_mut();
        inner_command.stdin(Stdio::piped());
        inner_command.stdout(Stdio::null());
        inner_command.stderr(Stdio::piped());

        // Log the FFmpeg command for debugging
        tracing::info!("[Encoder] FFmpeg command: {:?}", inner_command);

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
                    tracing::info!("[FFmpeg] {}", line);
                }
                tracing::info!("[FFmpeg] stderr reader thread exiting");
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

    /// Take ownership of the FFmpeg stdin handle.
    ///
    /// Returns `None` if stdin was already taken or the encoder wasn't started.
    /// Used to hand stdin to a dedicated writer thread so the pacing loop
    /// is never blocked by FFmpeg's stdin backpressure.
    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.video_stdin.take()
    }

    /// Get the output path.
    pub fn output_path(&self) -> &PathBuf {
        &self.output_path
    }

    /// Finalize the encoding and close the output file.
    ///
    /// Standard MP4 is written (not fragmented), so the moov atom will be
    /// written at the end when FFmpeg exits with correct duration.
    pub fn finish(mut self) -> Result<PathBuf, String> {
        tracing::info!("[Encoder] Closing FFmpeg stdin and waiting for process to finish...");

        // Close stdin to signal end of input
        drop(self.video_stdin.take());

        // Wait for FFmpeg to finish
        if let Some(mut child) = self.child.take() {
            let status = child
                .wait()
                .map_err(|e| format!("FFmpeg process error: {}", e))?;

            tracing::info!("[Encoder] FFmpeg exited with status: {:?}", status.code());

            if !status.success() {
                return Err(format!(
                    "FFmpeg encoding failed with exit code: {:?}",
                    status.code()
                ));
            }
        }

        // Check file size
        match std::fs::metadata(&self.output_path) {
            Ok(metadata) => {
                let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
                tracing::info!(
                    "[Encoder] Output file size: {:.2} MB ({:.2} KB)",
                    size_mb,
                    metadata.len() as f64 / 1024.0
                );
            }
            Err(e) => {
                tracing::warn!("[Encoder] Could not get file metadata: {}", e);
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

    #[cfg(target_os = "macos")]
    {
        let home = user_dirs.home_dir().to_path_buf();
        let recordings = home.join("Documents").join("Recordings");

        if recordings.exists() || std::fs::create_dir_all(&recordings).is_ok() {
            return Ok(recordings);
        }

        let documents = user_dirs
            .document_dir()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| home.join("Documents"));

        if documents.exists() || std::fs::create_dir_all(&documents).is_ok() {
            Ok(documents)
        } else {
            Ok(home)
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let output_dir = user_dirs
            .video_dir()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| {
                let home = user_dirs.home_dir().to_path_buf();
                let videos = home.join("Videos");
                if !videos.exists() && std::fs::create_dir_all(&videos).is_ok() {
                    return videos;
                }
                if videos.exists() {
                    videos
                } else {
                    home
                }
            });

        Ok(output_dir)
    }
}

/// Target frame rate for output video
const TARGET_FPS: u64 = 30;
/// Compute the exact time a given frame number should be written.
/// Uses multiplication (not accumulated addition) so there is zero
/// truncation drift regardless of recording length.
#[inline]
fn frame_time(start: Instant, frame_number: u64) -> Instant {
    start + Duration::from_nanos(frame_number * 1_000_000_000 / TARGET_FPS)
}
/// Helper: compute the signed difference between two `SystemTime` values
/// in seconds.  Returns `a - b` as a float (positive if a is after b).
fn systemtime_diff_secs(a: SystemTime, b: SystemTime) -> f64 {
    match a.duration_since(b) {
        Ok(d) => d.as_secs_f64(),
        Err(e) => -(e.duration().as_secs_f64()),
    }
}

/// Unified encoding function that receives video frames and optionally muxes
/// audio from vtx-engine's `RawAudioData` events in real-time.
///
/// # A/V Synchronization
///
/// Both audio and video are paced to wall-clock time.  Audio uses
/// `sample_offset` positioning with a `SystemTime`-based T=0 alignment.
/// Video uses a dedicated stdin writer thread so the pacing loop is never
/// blocked by FFmpeg's stdin backpressure (which occurs during H.264 codec
/// initialization and whenever the encoder falls behind).  The pacing loop
/// sends one frame per 33ms slot to a channel; the writer thread drains
/// the channel to FFmpeg's stdin.  Because both streams advance at
/// wall-clock rate, `frames_written / FPS ≈ samples_written / sample_rate`
/// and A/V sync is maintained.
pub fn encode_frames(
    mut frame_rx: mpsc::Receiver<CapturedFrame>,
    mut audio_rx: Option<broadcast::Receiver<EngineEvent>>,
    stop_flag: Arc<AtomicBool>,
    output_path: Option<PathBuf>,
    audio_capture_start: Option<SystemTime>,
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

    let frame_width = first_frame.width;
    let frame_height = first_frame.height;
    let frame_data_size = first_frame.data.len();
    tracing::info!(
        "[Encoder] Got first frame: {}x{}, data size: {} bytes",
        frame_width,
        frame_height,
        frame_data_size
    );

    // Set up audio pipe if audio is enabled.
    let mut audio_pipe = if has_audio {
        Some(AudioPipe::create()?)
    } else {
        None
    };

    // Create and start the video encoder
    let mut encoder = VideoEncoder::new_with_options(frame_width, frame_height, output_path)?;
    let pipe_path = audio_pipe.as_ref().map(|p| p.path().to_string());
    encoder.start(pipe_path.as_deref())?;

    tracing::debug!("[Encoder] Video output path: {:?}", encoder.output_path());

    // Write the first video frame BEFORE opening the audio pipe.
    // FFmpeg reads stdin (input 0) first, then opens the pipe (input 1).
    encoder.write_frame(&first_frame)?;

    // Take stdin out of the encoder so a dedicated writer thread can own it.
    // This decouples the pacing loop from FFmpeg's stdin backpressure.
    let video_stdin = encoder
        .take_stdin()
        .ok_or("Failed to take FFmpeg stdin for writer thread")?;

    // Open the audio pipe for writing (unblocks FFmpeg).
    if let Some(ref mut pipe) = audio_pipe {
        tracing::debug!("[Encoder] Opening audio pipe for writing: {}", pipe.path());
        pipe.open()?;
        tracing::debug!("[Encoder] Audio pipe opened");
    }

    // ── Timing reference ──
    //
    // Capture the wall-clock and monotonic references NOW, after all setup
    // (FFmpeg spawn, first frame write, pipe open) is done.  The pacing loop
    // starts from this instant.  Capturing earlier (e.g. at first-frame
    // receipt) would make the pacing loop think it's behind by the setup
    // duration, causing it to send extra frames and making video PTS
    // gradually drift ahead of audio PTS.
    let video_start_instant = Instant::now();
    let video_t0_system = SystemTime::now();

    tracing::debug!("[Encoder] Encoder initialized, entering main loop...");

    // ── Stdin writer thread ──
    //
    // Drains raw video frame data from a channel and writes it to FFmpeg's
    // stdin.  This thread may block during FFmpeg's codec initialization
    // (~4-7 seconds at high resolutions) -- that's fine, frames just queue
    // up in the channel.  The pacing loop on the main thread is never
    // blocked, so it keeps sending one frame per 33ms slot.
    //
    // After codec init, FFmpeg processes frames as fast as they arrive,
    // draining the backlog and then keeping pace with real-time.
    let (video_data_tx, video_data_rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(300); // ~10s buffer at 30fps
    let stdin_thread = {
        let mut stdin = video_stdin;
        std::thread::spawn(move || {
            let mut frames_written = 0u64;
            while let Ok(data) = video_data_rx.recv() {
                frames_written += 1;
                if stdin.write_all(&data).is_err() {
                    tracing::debug!("[StdinWriter] Write error, exiting");
                    break;
                }
            }
            tracing::info!(
                "[StdinWriter] Channel closed, wrote {} frames total",
                frames_written
            );
            drop(stdin); // Closes FFmpeg's stdin → signals EOF
        })
    };

    // ── Audio writer thread ──
    //
    // Writes freely based on sample_offset positioning.  Does NOT gate on
    // video frame count -- if audio stopped writing, FFmpeg would starve
    // on its audio input and stop reading video, deadlocking the pipeline.
    let audio_thread = if let Some(mut rx) = audio_rx.take() {
        let mut pipe = audio_pipe
            .take()
            .expect("audio_pipe must exist when audio_rx is Some");
        let audio_stop = stop_flag.clone();
        let audio_t0 = audio_capture_start.unwrap_or(video_t0_system);
        let video_t0 = video_t0_system;
        Some(std::thread::spawn(move || -> Result<u64, String> {
            let mut samples_written: u64 = 0;
            let mut sample_rate: u32 = 48000;
            let mut first_chunk = true;
            let mut initial_skip_samples: u64 = 0;
            let mut last_diag = Instant::now();

            let t0_offset_secs = systemtime_diff_secs(video_t0, audio_t0);

            tracing::info!(
                "[AudioWriter] Timeline: audio_t0 is {:.1}ms {} video_t0",
                t0_offset_secs.abs() * 1000.0,
                if t0_offset_secs >= 0.0 {
                    "before"
                } else {
                    "after"
                },
            );

            loop {
                if audio_stop.load(Ordering::Relaxed) {
                    loop {
                        match rx.try_recv() {
                            Ok(EngineEvent::RawAudioData(data)) => {
                                let pcm = f32_mono_to_s16le(&data.samples);
                                if pipe.write_all(&pcm).is_err() {
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

                let data = match rx.try_recv() {
                    Ok(EngineEvent::RawAudioData(d)) => d,
                    Ok(_) => continue,
                    Err(broadcast::error::TryRecvError::Empty) => {
                        std::thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(broadcast::error::TryRecvError::Lagged(n)) => {
                        tracing::warn!("[AudioWriter] Lagged: {} events skipped", n);
                        continue;
                    }
                    Err(broadcast::error::TryRecvError::Closed) => {
                        tracing::debug!("[AudioWriter] Broadcast channel closed");
                        break;
                    }
                };

                if first_chunk {
                    first_chunk = false;
                    sample_rate = data.sample_rate;

                    initial_skip_samples = if t0_offset_secs > 0.0 {
                        (t0_offset_secs * sample_rate as f64).round() as u64
                    } else {
                        0
                    };

                    if t0_offset_secs < 0.0 {
                        let pad = (-t0_offset_secs * sample_rate as f64).round() as u64;
                        if pad > 0 {
                            tracing::info!(
                                "[AudioWriter] Padding {:.1}ms silence ({} samples)",
                                -t0_offset_secs * 1000.0,
                                pad,
                            );
                            pipe.write_silence(pad as usize)?;
                            samples_written += pad;
                        }
                    }

                    tracing::info!(
                        "[AudioWriter] First chunk: offset={}, rate={}, \
                         skip={} ({:.0}ms)",
                        data.sample_offset,
                        data.sample_rate,
                        initial_skip_samples,
                        initial_skip_samples as f64 / sample_rate as f64 * 1000.0,
                    );
                }

                let target_pos = data.sample_offset.saturating_sub(initial_skip_samples);
                let chunk_len = data.samples.len() as u64;

                if target_pos + chunk_len <= samples_written {
                    continue;
                }

                if target_pos > samples_written {
                    let gap = target_pos - samples_written;
                    pipe.write_silence(gap as usize)?;
                    samples_written += gap;
                }

                let skip_front = if samples_written > target_pos {
                    (samples_written - target_pos) as usize
                } else {
                    0
                };
                let usable = &data.samples[skip_front.min(data.samples.len())..];
                if !usable.is_empty() {
                    let pcm = f32_mono_to_s16le(usable);
                    pipe.write_all(&pcm)?;
                    samples_written += usable.len() as u64;
                }

                let now_instant = Instant::now();
                if now_instant.duration_since(last_diag).as_secs() >= 5 {
                    let audio_pts = samples_written as f64 / sample_rate as f64;
                    let wall = systemtime_diff_secs(SystemTime::now(), video_t0);
                    tracing::info!(
                        "[AudioWriter] DIAG: wall={:.1}s audio_pts={:.1}s \
                         drift={:.1}ms written={}",
                        wall,
                        audio_pts,
                        (audio_pts - wall) * 1000.0,
                        samples_written,
                    );
                    last_diag = now_instant;
                }
            }

            drop(pipe);
            tracing::debug!(
                "[AudioWriter] Exiting: wrote {} samples ({:.1}s)",
                samples_written,
                samples_written as f64 / sample_rate as f64,
            );
            Ok(samples_written)
        }))
    } else {
        None
    };

    // ── Video pacing loop ──
    //
    // Sends exactly one frame per 33ms slot to the stdin writer channel.
    // Because channel sends are non-blocking (bounded channel with ~10s
    // capacity), this loop runs at wall-clock pace regardless of FFmpeg's
    // processing speed.  During codec init, frames queue in the channel.
    // After init, FFmpeg drains the backlog and then keeps pace.
    //
    // If the channel fills up (FFmpeg is severely behind), the send blocks
    // until FFmpeg catches up.  This is acceptable: it means the encoder
    // can't keep up with the source material, and back-pressure naturally
    // throttles the pacing loop.

    let enc_width = encoder.width;
    let enc_height = encoder.height;
    let mut frames_written = 1u64; // first frame already sent
    let mut last_frame = first_frame;
    let mut next_frame_time = frame_time(video_start_instant, frames_written);

    let mut consecutive_empty_polls = 0u32;
    const MAX_EMPTY_POLLS: u32 = 100;

    loop {
        let now = Instant::now();

        if stop_flag.load(Ordering::Relaxed) {
            tracing::debug!("[Encoder] Stop flag set, exiting loop");
            break;
        }

        // Receive latest video frame (non-blocking)
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

        // Send one frame per ~33.3ms slot (exact 30fps via frame_time()).
        if next_frame_time <= now {
            // Prepare frame data (handle cropping if frame is larger than encoder)
            let frame_data = if last_frame.width == enc_width && last_frame.height == enc_height {
                last_frame.data.clone()
            } else if last_frame.width >= enc_width && last_frame.height >= enc_height {
                // Crop to encoder dimensions
                let src_row_bytes = (last_frame.width * 4) as usize;
                let dst_row_bytes = (enc_width * 4) as usize;
                let mut cropped = Vec::with_capacity(dst_row_bytes * enc_height as usize);
                for y in 0..enc_height as usize {
                    let src_start = y * src_row_bytes;
                    let src_end = src_start + dst_row_bytes;
                    if src_end <= last_frame.data.len() {
                        cropped.extend_from_slice(&last_frame.data[src_start..src_end]);
                    }
                }
                cropped
            } else {
                // Frame too small -- skip this slot
                frames_written += 1;
                next_frame_time = frame_time(video_start_instant, frames_written);
                continue;
            };

            // Send to the stdin writer channel (blocks if full = backpressure).
            if video_data_tx.send(frame_data).is_err() {
                tracing::debug!("[Encoder] Stdin writer channel closed");
                break;
            }
            frames_written += 1;
            // Compute next frame time from frame count, not by adding an interval.
            // This eliminates truncation drift: frame N is always at exactly
            // N * 1_000_000_000 / 30 nanoseconds from start.
            next_frame_time = frame_time(video_start_instant, frames_written);

            // If still behind (e.g. after a channel-full stall), skip
            // ahead to the frame slot closest to `now`.
            if next_frame_time <= now {
                let elapsed_ns = now.duration_since(video_start_instant).as_nanos() as u64;
                let target_frame = elapsed_ns * TARGET_FPS / 1_000_000_000;
                if target_frame > frames_written {
                    let skip = target_frame - frames_written;
                    frames_written = target_frame;
                    next_frame_time = frame_time(video_start_instant, frames_written);
                    tracing::debug!("[Encoder] Pacing fell behind, skipped {} slots", skip);
                }
            }
        }

        // Sleep until next frame time
        let sleep_dur = next_frame_time.saturating_duration_since(Instant::now());
        if !sleep_dur.is_zero() {
            std::thread::sleep(sleep_dur.min(Duration::from_millis(10)));
        }
    }

    // Drain remaining frames from the capture channel
    tracing::info!("[Encoder] Draining remaining frames from capture channel...");
    let mut drain_count = 0u64;
    loop {
        match frame_rx.try_recv() {
            Ok(frame) => {
                // Prepare frame data (handle cropping if frame is larger than encoder)
                let frame_data = if frame.width == enc_width && frame.height == enc_height {
                    frame.data.clone()
                } else if frame.width >= enc_width && frame.height >= enc_height {
                    let src_row_bytes = (frame.width * 4) as usize;
                    let dst_row_bytes = (enc_width * 4) as usize;
                    let mut cropped = Vec::with_capacity(dst_row_bytes * enc_height as usize);
                    for y in 0..enc_height as usize {
                        let src_start = y * src_row_bytes;
                        let src_end = src_start + dst_row_bytes;
                        if src_end <= frame.data.len() {
                            cropped.extend_from_slice(&frame.data[src_start..src_end]);
                        }
                    }
                    cropped
                } else {
                    continue;
                };

                if video_data_tx.send(frame_data).is_ok() {
                    frames_written += 1;
                    drain_count += 1;
                } else {
                    break;
                }
            }
            Err(mpsc::error::TryRecvError::Empty) => break,
            Err(mpsc::error::TryRecvError::Disconnected) => break,
        }
    }
    if drain_count > 0 {
        tracing::info!("[Encoder] Drained {} additional frames", drain_count);
    }

    // Close the video data channel → stdin writer thread exits → stdin closes
    // → FFmpeg sees EOF on video input.
    tracing::info!("[Encoder] Closing video channel, waiting for stdin writer...");
    drop(video_data_tx);
    match stdin_thread.join() {
        Ok(_) => tracing::info!("[Encoder] Stdin writer thread finished successfully"),
        Err(e) => tracing::warn!("[Encoder] Stdin writer thread panicked: {:?}", e),
    }

    // Wait for the audio writer thread to finish and close the pipe.
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
        drop(audio_pipe);
        0
    };

    let elapsed = video_start_instant.elapsed().as_secs_f64();
    let video_pts = frames_written as f64 / TARGET_FPS as f64;
    tracing::info!(
        "[Encoder] Recording complete: {:.1}s wall, {:.1}s video PTS, \
         {} frames, {} audio samples",
        elapsed,
        video_pts,
        frames_written,
        audio_samples_written,
    );

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
