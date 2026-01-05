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

/// Whisper transcriber for audio-to-text conversion.
///
/// Uses the whisper.cpp library via FFI for efficient on-device transcription.
///
/// ## Model
///
/// OmniRec uses `ggml-medium.en` (~1.5GB) for better accuracy on longer segments,
/// unlike FlowSTT which uses the smaller `ggml-base.en` model for low latency.
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
    /// Returns the transcribed text, or "(No speech detected)" if the audio
    /// contains no recognizable speech.
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

        Ok(result)
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
}
