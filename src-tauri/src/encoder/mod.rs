//! Video encoding module using FFmpeg via ffmpeg-sidecar.

use crate::capture::recorder::CapturedFrame;
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
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let output_path = generate_output_path()?;

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
        // Skip frames with mismatched dimensions
        if frame.width != self.width || frame.height != self.height {
            eprintln!(
                "Skipping frame: dimensions {}x{} don't match encoder {}x{}",
                frame.width, frame.height, self.width, self.height
            );
            return Ok(());
        }

        if let Some(ref mut stdin) = self.stdin {
            // Handle stride: the buffer may have padding at the end of each row
            // We need to extract exactly width * 4 bytes per row
            let row_bytes = (self.width * 4) as usize;
            let buffer_row_bytes = frame.data.len() / self.height as usize;

            if buffer_row_bytes == row_bytes {
                // No stride padding, write directly
                stdin
                    .write_all(&frame.data)
                    .map_err(|e| format!("Failed to write frame: {}", e))?;
            } else {
                // Has stride padding, need to strip it row by row
                for y in 0..self.height as usize {
                    let row_start = y * buffer_row_bytes;
                    let row_end = row_start + row_bytes;
                    if row_end <= frame.data.len() {
                        stdin
                            .write_all(&frame.data[row_start..row_end])
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
            let output = child
                .wait()
                .map_err(|e| format!("FFmpeg process error: {}", e))?;

            if !output.success() {
                return Err(format!("FFmpeg encoding failed with exit code: {:?}", output.code()));
            }
        }

        Ok(self.output_path)
    }
}

/// Generate a unique output filename in the user's Videos folder.
fn generate_output_path() -> Result<PathBuf, String> {
    let user_dirs = UserDirs::new().ok_or("Could not determine user directories")?;
    let videos_dir = user_dirs
        .video_dir()
        .ok_or("Could not find Videos directory")?;

    let timestamp = Local::now().format("%Y-%m-%d_%H%M%S");
    let filename = format!("recording_{}.mp4", timestamp);

    Ok(videos_dir.join(filename))
}

/// Encoding task that receives frames from a channel and encodes them.
pub async fn encode_frames(
    mut frame_rx: mpsc::Receiver<CapturedFrame>,
    stop_flag: Arc<AtomicBool>,
) -> Result<PathBuf, String> {
    // Wait for first frame to get dimensions
    let first_frame = frame_rx
        .recv()
        .await
        .ok_or("No frames received")?;

    println!(
        "Starting encoder with dimensions {}x{}, frame data size: {}",
        first_frame.width, first_frame.height, first_frame.data.len()
    );

    let mut encoder = VideoEncoder::new(first_frame.width, first_frame.height)?;
    encoder.start()?;

    // Write first frame
    encoder.write_frame(&first_frame)?;

    let mut frame_count = 1u64;

    // Process remaining frames
    while let Some(frame) = frame_rx.recv().await {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }
        encoder.write_frame(&frame)?;
        frame_count += 1;
    }

    println!("Encoded {} frames total", frame_count);

    // Finalize
    encoder.finish()
}

/// Initialize FFmpeg (download if needed). Should be called once at app startup.
pub fn ensure_ffmpeg_blocking() -> Result<(), String> {
    ffmpeg_sidecar::download::auto_download()
        .map_err(|e| format!("Failed to download FFmpeg: {}", e))
}
