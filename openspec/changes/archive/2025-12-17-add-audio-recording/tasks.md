# Tasks: Add Audio Recording Support (Linux Only)

## 1. Audio Capture Infrastructure

- [x] 1.1 Define `AudioCaptureBackend` trait in `src-tauri/src/capture/mod.rs`
  - Methods: `list_audio_sources()`, `start_audio_capture(source_id)`, `stop_audio_capture()`
  - Audio sample receiver channel type
- [x] 1.2 Define `AudioSource` struct with id and display name
- [x] 1.3 Define `AudioSample` struct for PCM data (48kHz, stereo, 16-bit)

## 2. Linux Audio Capture (PipeWire)

- [x] 2.1 Create `src-tauri/src/capture/linux/audio.rs` module
- [x] 2.2 Implement `list_audio_sources()` - enumerate PipeWire audio sinks
- [x] 2.3 Implement `start_audio_capture(source_id)` - create PipeWire stream
- [x] 2.4 Implement `stop_audio_capture()` - stop stream and flush buffers
- [x] 2.5 Handle audio source disconnection during capture
- [ ] 2.6 Test audio capture with various PipeWire configurations

## 3. Platform Stubs

- [x] 3.1 Create `src-tauri/src/capture/windows/audio.rs` with stub implementation
  - `list_audio_sources()` returns empty vec
  - `start_audio_capture()` returns `NotImplemented` error
- [x] 3.2 Create `src-tauri/src/capture/macos/audio.rs` with stub implementation
  - `list_audio_sources()` returns empty vec
  - `start_audio_capture()` returns `NotImplemented` error

## 4. Configuration

- [x] 4.1 Add `AudioConfig` struct to `src-tauri/src/config.rs`
  - `enabled: bool` (default: true)
  - `source_id: Option<String>` (default: None)
- [x] 4.2 Add `audio` field to `AppConfig` struct
- [x] 4.3 Add Tauri commands: `get_audio_sources`, `save_audio_config`

## 5. FFmpeg Audio Integration

- [x] 5.1 Modify `VideoEncoder` to accept optional audio sample channel
- [x] 5.2 Update FFmpeg command to include audio input pipe when audio enabled
- [x] 5.3 Configure AAC encoding for audio track
- [x] 5.4 Handle video-only case (no audio pipe) gracefully
- [ ] 5.5 Test A/V sync with various recording durations

## 6. Recording State Management

- [x] 6.1 Add audio capture state to `RecordingManager`
- [x] 6.2 Start audio capture when recording starts (if enabled + source selected)
- [x] 6.3 Stop audio capture when recording stops
- [x] 6.4 Handle audio source disconnection during recording (continue video)

## 7. Frontend UI

- [x] 7.1 Add Audio settings group to configuration view in `index.html`
- [ ] 7.2 Add audio enable/disable toggle switch (simplified: removed - audio is enabled when a source is selected)
- [x] 7.3 Add audio source dropdown component
- [x] 7.4 Style audio controls in `src/styles.css`
- [x] 7.5 Implement audio config loading/saving in `src/main.ts`
- [x] 7.6 Handle audio source enumeration and refresh
- [ ] 7.7 Disable audio source dropdown when audio is disabled (N/A - simplified approach)
- [x] 7.8 Show "No audio sources found" when list is empty (handled by empty select)

## 8. Testing & Validation

- [ ] 8.1 Test video-only recording (audio disabled)
- [ ] 8.2 Test video-only recording (no audio source selected)
- [ ] 8.3 Test video+audio recording on Linux
- [ ] 8.4 Test A/V sync on recordings > 10 minutes
- [ ] 8.5 Test audio source disconnection mid-recording
- [ ] 8.6 Test transcoding with audio (WebM, MKV, etc.)
- [ ] 8.7 Verify output files play correctly in common players
- [ ] 8.8 Verify Windows/macOS builds still compile (stubs)

## 9. Documentation

- [ ] 9.1 Update README.md with audio recording feature (Linux only note)
