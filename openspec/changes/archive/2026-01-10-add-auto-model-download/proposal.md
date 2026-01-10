# Change: Add transcription model download and selection

## Why

Currently, users must manually download the whisper model file and place it in the correct location before transcription can work. This creates a poor first-run experience - users enable transcription, start recording, and only then discover it silently fails because the model is missing. Additionally, users are locked to the medium.en model with no option to choose smaller/faster models for quick transcriptions or larger models for better accuracy.

## What Changes

- Add model selection dropdown to transcription settings (tiny, base, small, medium, large-v3 for both English-only and multilingual variants)
- Add explicit "Download Model" button in settings UI
- Show download progress with percentage and cancel option
- Display model status (downloaded, not downloaded, downloading) in settings
- Update service to use the configured model instead of hardcoded medium.en
- Warn user if they try to record with transcription enabled but no model is present

## Impact

- Affected specs: `voice-transcription`, `app-configuration`
- Affected code:
  - `src-tauri/src/config.rs` - Add model selection to TranscriptionConfig
  - `src-tauri/src/commands/transcription.rs` - Add download/status commands
  - `src-service/src/transcription/transcriber.rs` - Use configured model path
  - `src/config.ts` - Model selection UI and download controls
  - `src/style.css` - Download progress styling
