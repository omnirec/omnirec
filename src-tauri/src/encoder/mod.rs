//! Video encoding module using FFmpeg via ffmpeg-sidecar.
//!
//! On Windows and macOS, FFmpeg is bundled as a Tauri sidecar binary alongside
//! the application. On Linux, the system-installed FFmpeg is used instead.

#![allow(dead_code)]

use crate::capture::types::{AudioSample, CapturedFrame};
use chrono::Local;
use directories::UserDirs;
use ffmpeg_sidecar::command::FfmpegCommand;
use std::io::Write;
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

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
            eprintln!("[Encoder] Failed to run ffmpeg -encoders: {}", e);
            String::new()
        }
    };

    eprintln!("[Encoder] Checking available H.264 encoders...");

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
            eprintln!("[Encoder] Found H.264 encoder: {}", encoder_name);
            return encoder_name;
        }
    }

    // Last resort fallback - try libx264 anyway
    eprintln!("[Encoder] Warning: No H.264 encoder detected in ffmpeg output!");
    eprintln!(
        "[Encoder] Available encoders: {}",
        encoders_output
            .lines()
            .filter(|l| l.contains("264") || l.contains("h264"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    eprintln!("[Encoder] Trying libx264 as fallback (may fail on Fedora without RPM Fusion)");
    "libx264"
}

/// Audio configuration for the encoder.
#[derive(Clone)]
pub struct AudioEncoderConfig {
    /// Sample rate in Hz (typically 48000)
    pub sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo)
    pub channels: u32,
}

impl Default for AudioEncoderConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
        }
    }
}

/// Video encoder that receives frames and encodes to MP4.
/// Optionally accepts audio input via a second pipe.
pub struct VideoEncoder {
    video_stdin: Option<ChildStdin>,
    audio_stdin: Option<ChildStdin>,
    child: Option<std::process::Child>,
    output_path: PathBuf,
    width: u32,
    height: u32,
    #[allow(dead_code)]
    audio_config: Option<AudioEncoderConfig>,
}

impl VideoEncoder {
    /// Create a new encoder with the given frame dimensions.
    /// Dimensions will be rounded down to even numbers for codec compatibility.
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        Self::new_with_options(width, height, None, None)
    }

    /// Create a new encoder with the given frame dimensions and optional audio.
    pub fn new_with_audio(
        width: u32,
        height: u32,
        audio_config: Option<AudioEncoderConfig>,
    ) -> Result<Self, String> {
        Self::new_with_options(width, height, audio_config, None)
    }

    /// Create a new encoder with the given frame dimensions, audio config, and optional output path.
    pub fn new_with_options(
        width: u32,
        height: u32,
        audio_config: Option<AudioEncoderConfig>,
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
            audio_stdin: None,
            child: None,
            output_path,
            width,
            height,
            audio_config,
        })
    }

    /// Start the FFmpeg encoding process.
    pub fn start(&mut self) -> Result<(), String> {
        // Detect available H.264 encoder
        let encoder = detect_h264_encoder();

        // Build the FFmpeg command using std::process for better stdin control
        let mut command = new_ffmpeg_command();
        command
            // Input: raw video frames from stdin
            .args(["-f", "rawvideo"])
            .args(["-pix_fmt", "bgra"])
            .args(["-s", &format!("{}x{}", self.width, self.height)])
            .args(["-r", "30"]) // 30 FPS
            .args(["-i", "-"]); // Read from stdin

        // Output: H.264 in MP4 container
        // Configure encoder-specific options
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
                eprintln!("[Encoder] Using generic options for encoder: {}", encoder);
            }
        }

        command
            .args(["-pix_fmt", "yuv420p"]) // Compatible pixel format
            .args(["-movflags", "+faststart"]) // Web-optimized MP4
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
                    eprintln!("[FFmpeg] {}", line);
                }
                eprintln!("[FFmpeg] stderr reader thread exiting");
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
            eprintln!(
                "Skipping frame: dimensions {}x{} smaller than encoder {}x{}",
                frame.width, frame.height, self.width, self.height
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
        drop(self.audio_stdin.take());

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

/// Audio encoder that writes PCM audio to a WAV file for later muxing.
pub struct AudioEncoder {
    file: Option<std::fs::File>,
    output_path: PathBuf,
    sample_rate: u32,
    channels: u32,
    bytes_written: u64,
}

impl AudioEncoder {
    /// Create a new audio encoder.
    pub fn new(sample_rate: u32, channels: u32) -> Result<Self, String> {
        let output_path =
            std::env::temp_dir().join(format!("omnirec_audio_{}.wav", std::process::id()));

        eprintln!("[AudioEncoder] Output path: {:?}", output_path);

        Ok(Self {
            file: None,
            output_path,
            sample_rate,
            channels,
            bytes_written: 0,
        })
    }

    /// Start the audio encoder (opens the file and writes WAV header placeholder).
    pub fn start(&mut self) -> Result<(), String> {
        let file = std::fs::File::create(&self.output_path)
            .map_err(|e| format!("Failed to create audio file: {}", e))?;

        // Write placeholder WAV header (44 bytes)
        // We'll update it with the correct size when we finish
        let header = create_wav_header(self.sample_rate, self.channels, 0);
        let mut file = file;
        file.write_all(&header)
            .map_err(|e| format!("Failed to write WAV header: {}", e))?;

        self.file = Some(file);
        Ok(())
    }

    /// Write audio samples to the encoder.
    /// Samples are expected as f32 values in the range [-1.0, 1.0].
    pub fn write_samples(&mut self, samples: &[f32]) -> Result<(), String> {
        if let Some(ref mut file) = self.file {
            // Convert f32 samples to 16-bit PCM
            let pcm_data: Vec<u8> = samples
                .iter()
                .flat_map(|&sample| {
                    // Clamp to [-1.0, 1.0] and convert to i16
                    let clamped = sample.clamp(-1.0, 1.0);
                    let value = (clamped * 32767.0) as i16;
                    value.to_le_bytes()
                })
                .collect();

            file.write_all(&pcm_data)
                .map_err(|e| format!("Failed to write audio samples: {}", e))?;

            self.bytes_written += pcm_data.len() as u64;
        }
        Ok(())
    }

    /// Finalize the audio encoding and return the output path.
    pub fn finish(mut self) -> Result<PathBuf, String> {
        if let Some(mut file) = self.file.take() {
            // Seek back to the beginning and update the WAV header with correct size
            use std::io::Seek;
            file.seek(std::io::SeekFrom::Start(0))
                .map_err(|e| format!("Failed to seek audio file: {}", e))?;

            let header =
                create_wav_header(self.sample_rate, self.channels, self.bytes_written as u32);
            file.write_all(&header)
                .map_err(|e| format!("Failed to update WAV header: {}", e))?;
        }

        eprintln!(
            "[AudioEncoder] Finished, wrote {} bytes of audio data",
            self.bytes_written
        );
        Ok(self.output_path)
    }

    /// Get the output path (for cleanup if encoding is cancelled).
    #[allow(dead_code)]
    pub fn output_path(&self) -> &PathBuf {
        &self.output_path
    }
}

/// Create a WAV file header.
fn create_wav_header(sample_rate: u32, channels: u32, data_size: u32) -> Vec<u8> {
    let byte_rate = sample_rate * channels * 2; // 16-bit samples
    let block_align = channels * 2;
    let file_size = 36 + data_size;

    let mut header = Vec::with_capacity(44);

    // RIFF header
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&file_size.to_le_bytes());
    header.extend_from_slice(b"WAVE");

    // fmt chunk
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes()); // Chunk size
    header.extend_from_slice(&1u16.to_le_bytes()); // Audio format (PCM)
    header.extend_from_slice(&(channels as u16).to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&byte_rate.to_le_bytes());
    header.extend_from_slice(&(block_align as u16).to_le_bytes());
    header.extend_from_slice(&16u16.to_le_bytes()); // Bits per sample

    // data chunk
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_size.to_le_bytes());

    header
}

/// Mux a video file and an audio file together.
/// `audio_delay_ms` is the delay of audio relative to video start (positive = audio started late).
/// Returns the path to the muxed output file.
pub fn mux_audio_video(
    video_path: &PathBuf,
    audio_path: &PathBuf,
    audio_delay_ms: i64,
) -> Result<PathBuf, String> {
    // Output to a new file with the same name as the video but with "_with_audio" suffix
    // Actually, let's just replace the video file
    let output_path = video_path.with_extension("_temp.mp4");

    eprintln!(
        "[Mux] Muxing video {:?} with audio {:?} (audio delay: {}ms)",
        video_path, audio_path, audio_delay_ms
    );

    let mut command = new_ffmpeg_command();
    command
        // Video input
        .args(["-i", video_path.to_string_lossy().as_ref()]);

    // Apply audio delay using -itsoffset BEFORE the audio input
    // Positive delay means audio started late, so we need to delay audio in the output
    // -itsoffset shifts the timestamps of the following input
    if audio_delay_ms != 0 {
        let delay_secs = audio_delay_ms as f64 / 1000.0;
        command.args(["-itsoffset", &format!("{:.3}", delay_secs)]);
    }

    command
        // Audio input (with offset applied)
        .args(["-i", audio_path.to_string_lossy().as_ref()])
        // Copy video stream (no re-encoding)
        .args(["-c:v", "copy"])
        // Encode audio to AAC
        .args(["-c:a", "aac"])
        .args(["-b:a", "192k"])
        // Map both streams
        .args(["-map", "0:v"]) // Video from first input
        .args(["-map", "1:a"]) // Audio from second input
        // Use shortest stream duration (in case of timing mismatch)
        .args(["-shortest"])
        // Output settings
        .args(["-movflags", "+faststart"])
        .args(["-y"])
        .arg(output_path.to_string_lossy().to_string());

    let inner_command = command.as_inner_mut();
    inner_command.stdout(Stdio::null());
    inner_command.stderr(Stdio::piped());

    let mut child = inner_command
        .spawn()
        .map_err(|e| format!("Failed to start FFmpeg for muxing: {}", e))?;

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
        .map_err(|e| format!("FFmpeg mux process error: {}", e))?;

    if !status.success() {
        let error_msg = if stderr_output.is_empty() {
            format!("FFmpeg muxing failed with exit code: {:?}", status.code())
        } else {
            format!(
                "FFmpeg muxing failed: {}",
                stderr_output.lines().last().unwrap_or(&stderr_output)
            )
        };
        return Err(error_msg);
    }

    // Replace the original video file with the muxed version
    std::fs::rename(&output_path, video_path)
        .map_err(|e| format!("Failed to replace video with muxed version: {}", e))?;

    // Clean up the audio file
    let _ = std::fs::remove_file(audio_path);

    eprintln!("[Mux] Successfully muxed audio and video");
    Ok(video_path.clone())
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

    eprintln!("[Encoder] Output path: {:?}", output_dir.join(&filename));
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

/// Encoding task that receives frames from a channel and encodes them.
/// Maintains consistent frame rate by duplicating frames when needed.
pub async fn encode_frames(
    mut frame_rx: mpsc::Receiver<CapturedFrame>,
    stop_flag: Arc<AtomicBool>,
) -> Result<PathBuf, String> {
    eprintln!("[Encoder] encode_frames task started, waiting for first frame...");

    // Wait for first frame to get dimensions
    let first_frame = frame_rx.recv().await.ok_or_else(|| {
        eprintln!("[Encoder] recv() returned None - channel closed without frames");
        "No frames received".to_string()
    })?;

    eprintln!(
        "[Encoder] Got first frame: {}x{}",
        first_frame.width, first_frame.height
    );

    eprintln!("[Encoder] Creating VideoEncoder...");
    let mut encoder = VideoEncoder::new(first_frame.width, first_frame.height).map_err(|e| {
        eprintln!("[Encoder] Failed to create encoder: {}", e);
        e
    })?;

    eprintln!("[Encoder] Starting FFmpeg...");
    encoder.start().map_err(|e| {
        eprintln!("[Encoder] Failed to start FFmpeg: {}", e);
        e
    })?;

    eprintln!("[Encoder] Writing first frame...");
    // Write first frame
    encoder.write_frame(&first_frame).map_err(|e| {
        eprintln!("[Encoder] Failed to write first frame: {}", e);
        e
    })?;

    eprintln!("[Encoder] Encoder initialized, entering main loop...");

    let mut frames_written = 1u64;
    let start_time = std::time::Instant::now();
    let mut last_frame = first_frame;
    let mut next_frame_time = start_time + std::time::Duration::from_millis(FRAME_INTERVAL_MS);

    // Process frames with timing
    // Track consecutive empty polls to detect when capture has truly stopped
    let mut consecutive_empty_polls = 0u32;
    const MAX_EMPTY_POLLS: u32 = 100; // ~1 second at 10ms per poll

    loop {
        let now = std::time::Instant::now();

        // Check stop flag
        if stop_flag.load(Ordering::Relaxed) {
            eprintln!("[Encoder] Stop flag set, exiting loop");
            break;
        }

        // Try to receive a new frame (non-blocking)
        match frame_rx.try_recv() {
            Ok(frame) => {
                last_frame = frame;
                consecutive_empty_polls = 0;
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                // No new frame available
                consecutive_empty_polls += 1;

                // If stop flag is set and we've had many empty polls, exit
                // This handles the case where the channel isn't properly closed
                if stop_flag.load(Ordering::Relaxed) && consecutive_empty_polls > 10 {
                    eprintln!("[Encoder] Stop flag set and no frames, exiting");
                    break;
                }

                // Safety exit if we've polled too many times with no frames
                if consecutive_empty_polls > MAX_EMPTY_POLLS {
                    eprintln!(
                        "[Encoder] No frames for {}ms, checking stop flag",
                        MAX_EMPTY_POLLS * 10
                    );
                    if stop_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    consecutive_empty_polls = 0; // Reset and continue
                }
            }
            Err(mpsc::error::TryRecvError::Disconnected) => {
                eprintln!("[Encoder] Channel disconnected, exiting loop");
                break;
            }
        }

        // Write frame(s) to maintain target FPS
        while next_frame_time <= now {
            encoder.write_frame(&last_frame)?;
            frames_written += 1;
            next_frame_time += std::time::Duration::from_millis(FRAME_INTERVAL_MS);
        }

        // Sleep until next frame time (with some margin for processing)
        let sleep_duration = next_frame_time.saturating_duration_since(std::time::Instant::now());
        if !sleep_duration.is_zero() {
            tokio::time::sleep(sleep_duration.min(std::time::Duration::from_millis(10))).await;
        }
    }

    let elapsed = start_time.elapsed().as_secs_f64();
    println!(
        "Recording complete: {:.1}s, {} frames",
        elapsed, frames_written
    );

    // Finalize
    encoder.finish()
}

/// Encoding task that receives video frames and audio samples, encoding them together.
/// Audio is recorded to a separate file and muxed at the end with proper timestamp alignment.
pub async fn encode_frames_with_audio(
    mut frame_rx: mpsc::Receiver<CapturedFrame>,
    mut audio_rx: mpsc::Receiver<AudioSample>,
    stop_flag: Arc<AtomicBool>,
    audio_config: AudioEncoderConfig,
) -> Result<PathBuf, String> {
    eprintln!("[Encoder] encode_frames_with_audio task started");

    // Wait for first video frame to get dimensions
    let first_frame = frame_rx
        .recv()
        .await
        .ok_or_else(|| "No video frames received".to_string())?;

    // Record the exact moment we received the first video frame
    // This is our reference point for A/V sync
    let video_start_time = std::time::Instant::now();

    eprintln!(
        "[Encoder] Got first frame: {}x{}",
        first_frame.width, first_frame.height
    );

    // Create video encoder
    let mut video_encoder = VideoEncoder::new(first_frame.width, first_frame.height)?;
    video_encoder.start()?;

    // Create audio encoder
    let mut audio_encoder = AudioEncoder::new(audio_config.sample_rate, audio_config.channels)?;
    audio_encoder.start()?;

    // Write first video frame
    video_encoder.write_frame(&first_frame)?;

    eprintln!("[Encoder] Encoders initialized, entering main loop...");

    let mut video_frames_written = 1u64;
    let mut audio_samples_written = 0u64;
    let mut last_frame = first_frame;
    let mut next_frame_time =
        video_start_time + std::time::Duration::from_millis(FRAME_INTERVAL_MS);

    // Track when first audio sample arrives (for A/V sync)
    let mut first_audio_time: Option<std::time::Instant> = None;

    let mut consecutive_empty_polls = 0u32;
    const MAX_EMPTY_POLLS: u32 = 100;

    loop {
        let now = std::time::Instant::now();

        // Check stop flag
        if stop_flag.load(Ordering::Relaxed) {
            eprintln!("[Encoder] Stop flag set, exiting loop");
            break;
        }

        // Process video frames
        match frame_rx.try_recv() {
            Ok(frame) => {
                last_frame = frame;
                consecutive_empty_polls = 0;
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                consecutive_empty_polls += 1;
                if stop_flag.load(Ordering::Relaxed) && consecutive_empty_polls > 10 {
                    eprintln!("[Encoder] Stop flag set and no frames, exiting");
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
                eprintln!("[Encoder] Video channel disconnected");
                break;
            }
        }

        // Process all available audio samples
        while let Ok(audio_sample) = audio_rx.try_recv() {
            // Record when first audio sample arrives
            if first_audio_time.is_none() {
                first_audio_time = Some(std::time::Instant::now());
                let delay_ms = first_audio_time
                    .unwrap()
                    .duration_since(video_start_time)
                    .as_millis();
                eprintln!(
                    "[Encoder] First audio sample received, delay from video start: {}ms",
                    delay_ms
                );
            }
            audio_encoder.write_samples(&audio_sample.data)?;
            audio_samples_written += audio_sample.data.len() as u64;
        }

        // Write video frames to maintain target FPS
        while next_frame_time <= now {
            video_encoder.write_frame(&last_frame)?;
            video_frames_written += 1;
            next_frame_time += std::time::Duration::from_millis(FRAME_INTERVAL_MS);
        }

        // Sleep until next frame time
        let sleep_duration = next_frame_time.saturating_duration_since(std::time::Instant::now());
        if !sleep_duration.is_zero() {
            tokio::time::sleep(sleep_duration.min(std::time::Duration::from_millis(10))).await;
        }
    }

    // Drain any remaining audio samples
    while let Ok(audio_sample) = audio_rx.try_recv() {
        if first_audio_time.is_none() {
            first_audio_time = Some(std::time::Instant::now());
        }
        audio_encoder.write_samples(&audio_sample.data)?;
        audio_samples_written += audio_sample.data.len() as u64;
    }

    let elapsed = video_start_time.elapsed().as_secs_f64();
    eprintln!(
        "[Encoder] Recording complete: {:.1}s, {} video frames, {} audio samples",
        elapsed, video_frames_written, audio_samples_written
    );

    // Finalize both encoders
    let video_path = video_encoder.finish()?;
    let audio_path = audio_encoder.finish()?;

    // Mux video and audio together
    if audio_samples_written > 0 {
        // Calculate audio delay relative to video start
        let audio_delay_ms = first_audio_time
            .map(|t| t.duration_since(video_start_time).as_millis() as i64)
            .unwrap_or(0);

        eprintln!(
            "[Encoder] Muxing audio and video (audio delay: {}ms)...",
            audio_delay_ms
        );
        mux_audio_video(&video_path, &audio_path, audio_delay_ms)?;
    } else {
        eprintln!("[Encoder] No audio recorded, keeping video-only");
        // Clean up empty audio file
        let _ = std::fs::remove_file(&audio_path);
    }

    Ok(video_path)
}

/// Encoding task with optional transcription support.
///
/// Similar to `encode_frames_with_audio`, but also sends audio samples to a transcription
/// channel for real-time voice transcription.
///
/// # Arguments
/// * `frame_rx` - Channel to receive video frames
/// * `audio_rx` - Channel to receive audio samples
/// * `stop_flag` - Flag to signal recording should stop
/// * `audio_config` - Audio encoder configuration
/// * `transcription_tx` - Optional channel to send audio samples for transcription
/// * `output_path` - Optional pre-generated output path (generated if None)
pub async fn encode_frames_with_audio_and_transcription(
    mut frame_rx: mpsc::Receiver<CapturedFrame>,
    mut audio_rx: mpsc::Receiver<AudioSample>,
    stop_flag: Arc<AtomicBool>,
    audio_config: AudioEncoderConfig,
    transcription_tx: Option<mpsc::Sender<Vec<f32>>>,
    output_path: Option<PathBuf>,
) -> Result<PathBuf, String> {
    eprintln!(
        "[Encoder] encode_frames_with_audio_and_transcription task started (transcription: {})",
        transcription_tx.is_some()
    );

    // Wait for first video frame to get dimensions
    let first_frame = frame_rx
        .recv()
        .await
        .ok_or_else(|| "No video frames received".to_string())?;

    // Record the exact moment we received the first video frame
    // This is our reference point for A/V sync
    let video_start_time = std::time::Instant::now();

    eprintln!(
        "[Encoder] Got first frame: {}x{}",
        first_frame.width, first_frame.height
    );

    // Create video encoder with optional output path
    let mut video_encoder = VideoEncoder::new_with_options(
        first_frame.width,
        first_frame.height,
        Some(audio_config.clone()),
        output_path,
    )?;
    video_encoder.start()?;
    eprintln!(
        "[Encoder] Video output path: {:?}",
        video_encoder.output_path()
    );

    // Create audio encoder
    let mut audio_encoder = AudioEncoder::new(audio_config.sample_rate, audio_config.channels)?;
    audio_encoder.start()?;

    // Write first video frame
    video_encoder.write_frame(&first_frame)?;

    eprintln!("[Encoder] Encoders initialized, entering main loop...");

    let mut video_frames_written = 1u64;
    let mut audio_samples_written = 0u64;
    let mut last_frame = first_frame;
    let mut next_frame_time =
        video_start_time + std::time::Duration::from_millis(FRAME_INTERVAL_MS);

    // Track when first audio sample arrives (for A/V sync)
    let mut first_audio_time: Option<std::time::Instant> = None;

    let mut consecutive_empty_polls = 0u32;
    const MAX_EMPTY_POLLS: u32 = 100;

    loop {
        let now = std::time::Instant::now();

        // Check stop flag
        if stop_flag.load(Ordering::Relaxed) {
            eprintln!("[Encoder] Stop flag set, exiting loop");
            break;
        }

        // Process video frames
        match frame_rx.try_recv() {
            Ok(frame) => {
                last_frame = frame;
                consecutive_empty_polls = 0;
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                consecutive_empty_polls += 1;
                if stop_flag.load(Ordering::Relaxed) && consecutive_empty_polls > 10 {
                    eprintln!("[Encoder] Stop flag set and no frames, exiting");
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
                eprintln!("[Encoder] Video channel disconnected");
                break;
            }
        }

        // Process all available audio samples
        while let Ok(audio_sample) = audio_rx.try_recv() {
            // Record when first audio sample arrives
            if first_audio_time.is_none() {
                first_audio_time = Some(std::time::Instant::now());
                let delay_ms = first_audio_time
                    .unwrap()
                    .duration_since(video_start_time)
                    .as_millis();
                eprintln!(
                    "[Encoder] First audio sample received, delay from video start: {}ms",
                    delay_ms
                );
            }

            // Write to audio encoder
            audio_encoder.write_samples(&audio_sample.data)?;
            audio_samples_written += audio_sample.data.len() as u64;

            // Fork samples to transcription if enabled
            if let Some(ref tx) = transcription_tx {
                // Non-blocking send - drop samples if queue is full
                match tx.try_send(audio_sample.data) {
                    Ok(()) => {}
                    Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                        // Log occasionally if queue is full
                        if audio_samples_written.is_multiple_of(100000) {
                            eprintln!("[Encoder] Transcription channel full, dropping samples");
                        }
                    }
                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                        eprintln!("[Encoder] Transcription channel closed");
                    }
                }
            }
        }

        // Write video frames to maintain target FPS
        while next_frame_time <= now {
            video_encoder.write_frame(&last_frame)?;
            video_frames_written += 1;
            next_frame_time += std::time::Duration::from_millis(FRAME_INTERVAL_MS);
        }

        // Sleep until next frame time
        let sleep_duration = next_frame_time.saturating_duration_since(std::time::Instant::now());
        if !sleep_duration.is_zero() {
            tokio::time::sleep(sleep_duration.min(std::time::Duration::from_millis(10))).await;
        }
    }

    // Drain any remaining audio samples
    while let Ok(audio_sample) = audio_rx.try_recv() {
        if first_audio_time.is_none() {
            first_audio_time = Some(std::time::Instant::now());
        }
        audio_encoder.write_samples(&audio_sample.data)?;
        audio_samples_written += audio_sample.data.len() as u64;

        // Fork to transcription
        if let Some(ref tx) = transcription_tx {
            let _ = tx.try_send(audio_sample.data);
        }
    }

    let elapsed = video_start_time.elapsed().as_secs_f64();
    eprintln!(
        "[Encoder] Recording complete: {:.1}s, {} video frames, {} audio samples",
        elapsed, video_frames_written, audio_samples_written
    );

    // Finalize both encoders
    let video_path = video_encoder.finish()?;
    let audio_path = audio_encoder.finish()?;

    // Mux video and audio together
    if audio_samples_written > 0 {
        // Calculate audio delay relative to video start
        let audio_delay_ms = first_audio_time
            .map(|t| t.duration_since(video_start_time).as_millis() as i64)
            .unwrap_or(0);

        eprintln!(
            "[Encoder] Muxing audio and video (audio delay: {}ms)...",
            audio_delay_ms
        );
        mux_audio_video(&video_path, &audio_path, audio_delay_ms)?;
    } else {
        eprintln!("[Encoder] No audio recorded, keeping video-only");
        // Clean up empty audio file
        let _ = std::fs::remove_file(&audio_path);
    }

    Ok(video_path)
}

/// Ensure FFmpeg is available. Should be called once at app startup.
///
/// On Windows and macOS, verifies that the bundled sidecar binary exists and is
/// executable. On Linux, falls back to runtime auto-download if the system
/// FFmpeg is not available (though it should be installed as a package dependency).
pub fn ensure_ffmpeg_blocking() -> Result<(), String> {
    let ffmpeg = resolve_ffmpeg_path();
    eprintln!("[FFmpeg] Resolved path: {}", ffmpeg.display());

    // Verify the binary is accessible by running `ffmpeg -version`
    match Command::new(&ffmpeg)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {
            eprintln!("[FFmpeg] Binary verified OK");
            Ok(())
        }
        Ok(status) => Err(format!(
            "FFmpeg binary at {} exited with status: {}",
            ffmpeg.display(),
            status
        )),
        Err(e) => {
            eprintln!(
                "[FFmpeg] Binary not found at {}: {}",
                ffmpeg.display(),
                e
            );
            // On Linux, try auto-download as a last resort (system package may
            // not be installed in development environments)
            #[cfg(target_os = "linux")]
            {
                eprintln!("[FFmpeg] Attempting auto-download as fallback...");
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

use omnirec_common::OutputFormat;
use std::path::Path;

/// Transcode a source MP4 file to the specified output format.
/// Returns the path to the transcoded file.
pub fn transcode_video(source_path: &Path, format: OutputFormat) -> Result<PathBuf, String> {
    // Generate output path with new extension
    let output_path = source_path.with_extension(format.extension());

    eprintln!(
        "[Transcode] Converting {} to {:?}",
        source_path.display(),
        format
    );
    eprintln!("[Transcode] Output: {}", output_path.display());

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

    eprintln!("[Transcode] Successfully created {}", output_path.display());
    Ok(output_path)
}
