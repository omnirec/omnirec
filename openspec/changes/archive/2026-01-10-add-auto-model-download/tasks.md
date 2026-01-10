## 1. Backend - Configuration

- [x] 1.1 Add `WhisperModel` enum with all supported models (tiny, base, small, medium, large-v3) and language variants (.en suffix)
- [x] 1.2 Add `model` field to `TranscriptionConfig` struct with default `medium.en`
- [x] 1.3 Add helper functions for model metadata (filename, size, download URL)
- [x] 1.4 Update config serialization tests

## 2. Backend - Model Management Commands

- [x] 2.1 Add `get_model_status` command returning: model name, expected path, exists flag, file size
- [x] 2.2 Add `download_model` command that downloads selected model with progress events
- [x] 2.3 Add `cancel_download` command to abort in-progress download
- [x] 2.4 Add `list_available_models` command returning model info (name, size, description)
- [x] 2.5 Emit `model-download-progress` events with percentage during download

## 3. Backend - Service Integration

- [x] 3.1 Update service IPC to accept model path from config
- [x] 3.2 Modify transcriber to use configured model path instead of hardcoded default
- [ ] 3.3 Add model availability check before starting transcription

## 4. Frontend - Settings UI

- [x] 4.1 Add model selection dropdown in transcription settings section
- [x] 4.2 Show model size hint next to each option (e.g., "medium.en (1.5 GB)")
- [x] 4.3 Add model status indicator (Downloaded / Not Downloaded / Downloading)
- [x] 4.4 Add "Download" button when model not present
- [x] 4.5 Add progress bar during download with percentage
- [x] 4.6 Add "Cancel" button during download
- [x] 4.7 Disable model dropdown during download
- [x] 4.8 Style download progress bar and status indicators

## 5. Frontend - Recording Integration

- [x] 5.1 Check model status when starting recording with transcription enabled
- [x] 5.2 Show warning notification if model not downloaded
- [x] 5.3 Prevent recording start if transcription enabled but model missing (with clear message)

## 6. Testing

- [x] 6.1 Add unit tests for model metadata functions
- [ ] 6.2 Add integration tests for download command (mock HTTP)
- [ ] 6.3 Manual test: Download model, verify transcription works
- [ ] 6.4 Manual test: Change model, verify new model is used
- [ ] 6.5 Manual test: Cancel download mid-progress

## 7. Documentation

- [ ] 7.1 Update README with model selection information
- [ ] 7.2 Update CLI docs if model commands are exposed
