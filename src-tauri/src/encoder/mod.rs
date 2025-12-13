//! Video encoding module using FFmpeg via ffmpeg-sidecar.

use crate::capture::CapturedFrame;
use chrono::Local;
use directories::UserDirs;
use ffmpeg_sidecar::command::FfmpegCommand;
use std::io::Write;
use std::path::PathBuf;
use std::process::{ChildStdin, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Video encoder that receives frames and encodes to MP4.
pub struct VideoEncoder {
    stdin: Option<ChildStdin>,
    child: Option<std::process::Child>,
    output_path: PathBuf,
    width: u32,
    height: u32,
}

impl VideoEncoder {
    /// Create a new encoder with the given frame dimensions.
    /// Dimensions will be rounded down to even numbers for codec compatibility.
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let output_path = generate_output_path()?;

        // Ensure dimensions are even (required by many codecs including h264)
        let width = width & !1;
        let height = height & !1;

        if width == 0 || height == 0 {
            return Err(format!("Invalid dimensions: {}x{}", width, height));
        }

        Ok(Self {
            stdin: None,
            child: None,
            output_path,
            width,
            height,
        })
    }

    /// Start the FFmpeg encoding process.
    pub fn start(&mut self) -> Result<(), String> {
        // Build the FFmpeg command using std::process for better stdin control
        let mut command = FfmpegCommand::new();
        command
            // Input: raw video frames from stdin
            .args(["-f", "rawvideo"])
            .args(["-pix_fmt", "bgra"])
            .args(["-s", &format!("{}x{}", self.width, self.height)])
            .args(["-r", "30"]) // 30 FPS
            .args(["-i", "-"]) // Read from stdin
            // Output: H.264 in MP4 container
            .args(["-c:v", "libx264"])
            .args(["-preset", "ultrafast"]) // Fast encoding for real-time
            .args(["-crf", "23"]) // Good quality/size balance
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

        let stdin = child
            .stdin
            .take()
            .ok_or("Failed to get FFmpeg stdin")?;

        self.stdin = Some(stdin);
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

        if let Some(ref mut stdin) = self.stdin {
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

    /// Finalize the encoding and close the output file.
    pub fn finish(mut self) -> Result<PathBuf, String> {
        // Close stdin to signal end of input
        drop(self.stdin.take());

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
                    format!("FFmpeg failed: {}", stderr_output.lines().last().unwrap_or(&stderr_output))
                };
                return Err(error_msg);
            }
        }

        Ok(self.output_path)
    }
}

/// Generate a unique output filename in the user's Videos folder.
fn generate_output_path() -> Result<PathBuf, String> {
    let user_dirs = UserDirs::new().ok_or("Could not determine user directories")?;
    
    // Try Videos directory first, fall back to home directory
    let output_dir = user_dirs
        .video_dir()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| {
            let home = user_dirs.home_dir().to_path_buf();
            let videos = home.join("Videos");
            // Try to create Videos directory if it doesn't exist
            if !videos.exists() {
                if std::fs::create_dir_all(&videos).is_ok() {
                    return videos;
                }
            }
            // Fall back to home directory
            home
        });

    let timestamp = Local::now().format("%Y-%m-%d_%H%M%S");
    let filename = format!("recording_{}.mp4", timestamp);

    Ok(output_dir.join(filename))
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
    let first_frame = frame_rx
        .recv()
        .await
        .ok_or_else(|| {
            eprintln!("[Encoder] recv() returned None - channel closed without frames");
            "No frames received".to_string()
        })?;
    
    eprintln!("[Encoder] Got first frame: {}x{}", first_frame.width, first_frame.height);

    eprintln!("[Encoder] Creating VideoEncoder...");
    let mut encoder = VideoEncoder::new(first_frame.width, first_frame.height)
        .map_err(|e| {
            eprintln!("[Encoder] Failed to create encoder: {}", e);
            e
        })?;
    
    eprintln!("[Encoder] Starting FFmpeg...");
    encoder.start()
        .map_err(|e| {
            eprintln!("[Encoder] Failed to start FFmpeg: {}", e);
            e
        })?;

    eprintln!("[Encoder] Writing first frame...");
    // Write first frame
    encoder.write_frame(&first_frame)
        .map_err(|e| {
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
                    eprintln!("[Encoder] No frames for {}ms, checking stop flag", MAX_EMPTY_POLLS * 10);
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
    println!("Recording complete: {:.1}s, {} frames", elapsed, frames_written);

    // Finalize
    encoder.finish()
}

/// Initialize FFmpeg (download if needed). Should be called once at app startup.
pub fn ensure_ffmpeg_blocking() -> Result<(), String> {
    ffmpeg_sidecar::download::auto_download()
        .map_err(|e| format!("Failed to download FFmpeg: {}", e))
}

use crate::state::OutputFormat;

/// Transcode a source MP4 file to the specified output format.
/// Returns the path to the transcoded file.
pub fn transcode_video(source_path: &PathBuf, format: OutputFormat) -> Result<PathBuf, String> {
    // Generate output path with new extension
    let output_path = source_path.with_extension(format.extension());
    
    eprintln!("[Transcode] Converting {} to {:?}", source_path.display(), format);
    eprintln!("[Transcode] Output: {}", output_path.display());

    let mut command = FfmpegCommand::new();
    
    // Input file
    command.args(["-i", source_path.to_string_lossy().as_ref()]);
    
    // Format-specific encoding settings
    match format {
        OutputFormat::Mp4 => {
            // No transcoding needed for MP4
            return Ok(source_path.clone());
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
            format!("FFmpeg transcoding failed with exit code: {:?}", status.code())
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
