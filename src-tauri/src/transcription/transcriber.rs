//! Whisper transcription wrapper.
//!
//! This module provides a high-level wrapper around the whisper.cpp FFI bindings
//! for transcribing audio segments.
//!
//! Ported from FlowSTT's `transcribe.rs` and configured for OmniRec's longer segments.

use std::path::PathBuf;

use super::whisper_ffi::{self, Context, WhisperSamplingStrategy};

/// URL for downloading the medium.en model (used by OmniRec for better accuracy)
#[allow(dead_code)]
const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.en.bin";

/// Minimum number of repetitions to consider text as a hallucination loop
const MIN_REPETITIONS_FOR_LOOP: usize = 3;

/// Minimum phrase length (in chars) to check for repetition
const MIN_PHRASE_LENGTH: usize = 10;

/// Whisper transcriber for audio-to-text conversion.
///
/// Uses the whisper.cpp library via FFI for efficient on-device transcription.
///
/// ## Model
///
/// OmniRec uses `ggml-medium.en` (~1.5GB) for better accuracy on longer segments,
/// unlike FlowSTT which uses the smaller `ggml-base.en` model for low latency.
///
/// ## Hallucination Mitigation
///
/// Whisper can sometimes produce repetition loops where the same phrase is
/// repeated many times. This transcriber includes post-processing to detect
/// and remove such loops, keeping only the first occurrence.
pub struct Transcriber {
    /// Whisper context (lazily initialized)
    ctx: Option<Context>,
    /// Path to the model file
    model_path: PathBuf,
    /// Whether the library has been initialized
    library_initialized: bool,
}

impl Transcriber {
    /// Create a new transcriber with the default model path.
    pub fn new() -> Self {
        let model_path = whisper_ffi::get_default_model_path();
        Self {
            ctx: None,
            model_path,
            library_initialized: false,
        }
    }

    /// Create a transcriber with a custom model path.
    pub fn with_model_path(model_path: PathBuf) -> Self {
        Self {
            ctx: None,
            model_path,
            library_initialized: false,
        }
    }

    /// Get the model path.
    #[allow(dead_code)]
    pub fn model_path(&self) -> &PathBuf {
        &self.model_path
    }

    /// Check if the model file exists.
    pub fn is_model_available(&self) -> bool {
        self.model_path.exists()
    }

    /// Check if the model is loaded.
    #[allow(dead_code)]
    pub fn is_model_loaded(&self) -> bool {
        self.ctx.is_some()
    }

    /// Ensure the whisper library is initialized.
    fn ensure_library(&mut self) -> Result<(), String> {
        if !self.library_initialized {
            whisper_ffi::init_library()?;
            self.library_initialized = true;
        }
        Ok(())
    }

    /// Load the whisper model.
    ///
    /// This is called automatically by `transcribe()` if the model isn't loaded,
    /// but can be called explicitly to pre-warm the model.
    pub fn load_model(&mut self) -> Result<(), String> {
        if self.ctx.is_some() {
            return Ok(());
        }

        self.ensure_library()?;

        if !self.model_path.exists() {
            return Err(format!(
                "Whisper model not found at: {}\n\n\
                Please download the model file:\n\
                1. Visit: https://huggingface.co/ggerganov/whisper.cpp/tree/main\n\
                2. Download 'ggml-medium.en.bin' (~1.5GB)\n\
                3. Place it at: {}",
                self.model_path.display(),
                self.model_path.display()
            ));
        }

        tracing::info!("Loading whisper model from: {}", self.model_path.display());
        let ctx = Context::new(&self.model_path)?;
        self.ctx = Some(ctx);
        tracing::info!("Whisper model loaded successfully");

        Ok(())
    }

    /// Transcribe audio samples to text.
    ///
    /// The audio should be mono 16kHz f32 samples (whisper's expected format).
    /// Returns the transcribed text, or an empty string if the audio
    /// contains no recognizable speech.
    ///
    /// The output is post-processed to remove hallucination loops (repeated phrases).
    pub fn transcribe(&mut self, audio_data: &[f32]) -> Result<String, String> {
        self.load_model()?;

        let ctx = self.ctx.as_ref().unwrap();

        // Get default params with greedy strategy
        let mut params = whisper_ffi::full_default_params(WhisperSamplingStrategy::Greedy)?;

        // Configure for longer audio segments (OmniRec tuning)
        params.configure_for_long_audio();

        // Run transcription
        ctx.full(&params, audio_data)?;

        let num_segments = ctx.full_n_segments()?;

        if num_segments == 0 {
            return Ok(String::new());
        }

        let mut result = String::new();
        for i in 0..num_segments {
            if let Ok(segment) = ctx.full_get_segment_text(i) {
                let trimmed = segment.trim();
                if !trimmed.is_empty() {
                    if !result.is_empty() {
                        result.push(' ');
                    }
                    result.push_str(trimmed);
                }
            }
        }

        // Post-process to remove hallucination loops
        let result = Self::remove_repetition_loops(&result);

        Ok(result)
    }

    /// Remove repetition loops (hallucinations) from transcribed text.
    ///
    /// Whisper sometimes produces output like:
    /// "And I think that's important. And I think that's important. And I think that's important."
    ///
    /// This function detects such patterns and keeps only the first occurrence.
    fn remove_repetition_loops(text: &str) -> String {
        if text.len() < MIN_PHRASE_LENGTH * MIN_REPETITIONS_FOR_LOOP {
            return text.to_string();
        }

        // Split into sentences/phrases for analysis
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.len() < MIN_REPETITIONS_FOR_LOOP * 3 {
            return text.to_string();
        }

        // Try to find repeating word sequences of different lengths
        // Start with longer sequences (more reliable detection)
        for seq_len in (3..=words.len() / MIN_REPETITIONS_FOR_LOOP).rev() {
            if let Some(result) = Self::find_and_remove_word_sequence_repetition(&words, seq_len) {
                tracing::debug!(
                    "Removed repetition loop (seq_len={}): '{}' -> '{}'",
                    seq_len,
                    text,
                    result
                );
                return result;
            }
        }

        text.to_string()
    }

    /// Find repeating word sequences and remove duplicates.
    fn find_and_remove_word_sequence_repetition(words: &[&str], seq_len: usize) -> Option<String> {
        if words.len() < seq_len * MIN_REPETITIONS_FOR_LOOP {
            return None;
        }

        // Try each starting position
        for start in 0..=(words.len() - seq_len * MIN_REPETITIONS_FOR_LOOP) {
            let pattern: Vec<&str> = words[start..start + seq_len].to_vec();
            let pattern_lower: Vec<String> = pattern.iter().map(|w| w.to_lowercase()).collect();

            // Count consecutive occurrences of this pattern
            let mut count = 1;
            let mut pos = start + seq_len;

            while pos + seq_len <= words.len() {
                let candidate: Vec<String> = words[pos..pos + seq_len]
                    .iter()
                    .map(|w| w.to_lowercase())
                    .collect();

                if candidate == pattern_lower {
                    count += 1;
                    pos += seq_len;
                } else {
                    break;
                }
            }

            // Found a repetition loop
            if count >= MIN_REPETITIONS_FOR_LOOP {
                // Build result: words before pattern + single pattern + words after repetitions
                let mut result_words: Vec<&str> = Vec::new();

                // Add words before the pattern
                result_words.extend_from_slice(&words[..start]);

                // Add the pattern once (use original casing from first occurrence)
                result_words.extend_from_slice(&pattern);

                // Add words after all repetitions
                let after_repetitions = start + seq_len * count;
                if after_repetitions < words.len() {
                    result_words.extend_from_slice(&words[after_repetitions..]);
                }

                return Some(result_words.join(" "));
            }
        }

        None
    }

    /// Get whisper.cpp system info (available backends, etc.)
    #[allow(dead_code)]
    pub fn get_system_info(&self) -> Result<String, String> {
        whisper_ffi::get_system_info()
    }
}

impl Default for Transcriber {
    fn default() -> Self {
        Self::new()
    }
}

/// Download the whisper model to the specified path.
///
/// This downloads the medium.en model (~1.5GB) from Hugging Face.
#[allow(dead_code)]
pub fn download_model(model_path: &PathBuf) -> Result<(), String> {
    use std::fs;
    use std::io::Write;

    // Create parent directory if it doesn't exist
    if let Some(parent) = model_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    tracing::info!("Downloading whisper model to: {}", model_path.display());
    tracing::info!("This may take a while (~1.5GB)...");

    // Download the model
    let response = reqwest::blocking::get(MODEL_URL)
        .map_err(|e| format!("Failed to download model: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to download model: HTTP {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // Write to file
    let mut file =
        fs::File::create(model_path).map_err(|e| format!("Failed to create file: {}", e))?;

    file.write_all(&bytes)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    tracing::info!("Model download complete");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcriber_creation() {
        let transcriber = Transcriber::new();
        // Model path should be set to default
        assert!(!transcriber.model_path().as_os_str().is_empty());
    }

    #[test]
    fn test_custom_model_path() {
        let path = PathBuf::from("/custom/path/model.bin");
        let transcriber = Transcriber::with_model_path(path.clone());
        assert_eq!(transcriber.model_path(), &path);
    }

    #[test]
    fn test_remove_repetition_loops_basic() {
        // Classic hallucination loop
        let input = "And I think that's a very important point. And I think that's a very important point. And I think that's a very important point. And I think that's a very important point.";
        let result = Transcriber::remove_repetition_loops(input);
        assert!(
            result
                .matches("And I think that's a very important point")
                .count()
                == 1,
            "Expected single occurrence, got: {}",
            result
        );
    }

    #[test]
    fn test_remove_repetition_loops_with_trailing() {
        // Hallucination with text after (need at least 3 words per phrase)
        let input =
            "This is important. This is important. This is important. And then something else.";
        let result = Transcriber::remove_repetition_loops(input);
        // Should keep first occurrence and trailing text
        assert!(result.contains("This is important"));
        assert!(result.contains("something else"));
        assert!(
            result.matches("This is important").count() == 1,
            "Expected single occurrence, got: {}",
            result
        );
    }

    #[test]
    fn test_remove_repetition_loops_no_repetition() {
        // Normal text without repetition
        let input = "This is a normal sentence. And this is another one. Nothing repeating here.";
        let result = Transcriber::remove_repetition_loops(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_remove_repetition_loops_short_text() {
        // Text too short to be a loop
        let input = "Short text.";
        let result = Transcriber::remove_repetition_loops(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_remove_repetition_loops_two_occurrences_ok() {
        // Two occurrences is not enough to be considered a loop
        let input = "I agree. I agree.";
        let result = Transcriber::remove_repetition_loops(input);
        // Should not be modified (only 2 occurrences, below threshold)
        assert_eq!(result, input);
    }
}
