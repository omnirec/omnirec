//! Transcript file writer.
//!
//! This module writes transcribed segments to a markdown file with timestamps.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// Transcript writer for markdown output.
///
/// Writes transcribed segments with timestamps in the format:
/// ```markdown
/// [HH:MM:SS] transcribed text
/// ```
pub struct TranscriptWriter {
    /// Path to the transcript file
    path: PathBuf,
    /// Buffered writer for the file
    writer: BufWriter<File>,
    /// Whether the header has been written
    header_written: bool,
}

impl TranscriptWriter {
    /// Create a new transcript writer.
    ///
    /// Creates or truncates the transcript file at the given path.
    pub fn new(path: &Path) -> Result<Self, String> {
        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .map_err(|e| format!("Failed to create transcript file: {}", e))?;

        let writer = BufWriter::new(file);

        Ok(Self {
            path: path.to_path_buf(),
            writer,
            header_written: false,
        })
    }

    /// Write the header if not already written.
    fn ensure_header(&mut self) -> Result<(), String> {
        if self.header_written {
            return Ok(());
        }

        // Write markdown header
        writeln!(self.writer, "# Recording Transcript\n")
            .map_err(|e| format!("Failed to write header: {}", e))?;

        self.header_written = true;
        Ok(())
    }

    /// Write a transcribed segment with timestamp.
    ///
    /// # Arguments
    /// * `timestamp_secs` - Timestamp in seconds from recording start
    /// * `text` - The transcribed text
    pub fn write_segment(&mut self, timestamp_secs: f64, text: &str) -> Result<(), String> {
        self.ensure_header()?;

        let formatted_time = format_timestamp(timestamp_secs);

        writeln!(self.writer, "[{}] {}\n", formatted_time, text.trim())
            .map_err(|e| format!("Failed to write segment: {}", e))?;

        // Flush to ensure data is written
        self.writer
            .flush()
            .map_err(|e| format!("Failed to flush: {}", e))?;

        Ok(())
    }

    /// Finalize the transcript file.
    ///
    /// Flushes any remaining buffered data and closes the file.
    pub fn finalize(&mut self) -> Result<(), String> {
        self.writer
            .flush()
            .map_err(|e| format!("Failed to finalize transcript: {}", e))?;

        tracing::info!("Transcript saved to: {}", self.path.display());
        Ok(())
    }

    /// Get the path to the transcript file.
    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Format a timestamp in seconds as HH:MM:SS.
fn format_timestamp(seconds: f64) -> String {
    let total_seconds = seconds.round() as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let secs = total_seconds % 60;

    format!("{:02}:{:02}:{:02}", hours, minutes, secs)
}

/// Generate transcript filename from video filename.
///
/// Given a video filename like "recording_2024-01-15_14-30-00.mp4",
/// returns "recording_2024-01-15_14-30-00_transcript.md".
pub fn transcript_filename_from_video(video_path: &Path) -> PathBuf {
    let stem = video_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("recording");

    let mut transcript_path = video_path.to_path_buf();
    transcript_path.set_file_name(format!("{}_transcript.md", stem));

    transcript_path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(0.0), "00:00:00");
        assert_eq!(format_timestamp(59.0), "00:00:59");
        assert_eq!(format_timestamp(60.0), "00:01:00");
        assert_eq!(format_timestamp(3661.0), "01:01:01");
        assert_eq!(format_timestamp(7322.5), "02:02:03"); // Rounds to 7323
    }

    #[test]
    fn test_transcript_filename() {
        let video = PathBuf::from("/path/to/recording_2024-01-15.mp4");
        let transcript = transcript_filename_from_video(&video);
        assert_eq!(
            transcript,
            PathBuf::from("/path/to/recording_2024-01-15_transcript.md")
        );
    }
}
