## 1. Dependencies

- [x] 1.1 Add `aec3 = "0.1"` dependency to `src-tauri/Cargo.toml`

## 2. Configuration Layer

- [x] 2.1 Add `microphone_id: Option<String>` field to `AudioConfig` struct
- [x] 2.2 Add `echo_cancellation: bool` field to `AudioConfig` struct (default: true)
- [x] 2.3 Update config serialization/deserialization to handle new fields
- [x] 2.4 Add unit tests for new config fields

## 3. Audio Mixer

- [x] 3.1 Create `AudioMixer` struct with dual-stream buffering (see flowstt reference)
- [x] 3.2 Add buffers for microphone and system audio samples
- [x] 3.3 Initialize AEC3 via `VoipAec3::builder(48000, 2, 2).enable_high_pass(true).build()`
- [x] 3.4 Process samples in AEC-compatible frame sizes (480 samples/channel = 10ms at 48kHz)
- [x] 3.5 Implement mixing with 0.5 weighting per source + clipping protection
- [x] 3.6 Handle single-stream pass-through (when only one source active)
- [ ] 3.7 Add unit tests for mixing logic

## 4. AEC3 Integration

- [x] 4.1 Call `aec.process(&mic_samples, Some(&ref_samples), false, &mut out)` for echo cancellation
- [x] 4.2 Use system audio as render (reference) signal, microphone as capture signal
- [x] 4.3 Skip AEC processing when `echo_cancellation` config is false
- [x] 4.4 Skip AEC when only microphone is active (no reference signal)
- [x] 4.5 Add logging for AEC status (enabled/disabled/error)

## 5. Audio Capture - Dual Stream Support

- [x] 5.1 Refactor `start_audio_capture` to accept optional second source ID for microphone
- [x] 5.2 Create second PipeWire capture stream for microphone input
- [x] 5.3 Route microphone samples to mixer buffer 1, system audio to buffer 2
- [x] 5.4 Handle mono-to-stereo conversion for mono microphones

## 6. Recording Pipeline Integration

- [x] 6.1 Update `start_audio_capture_for_recording` to pass microphone config
- [x] 6.2 Pass AEC enabled flag to mixer (shared `Arc<Mutex<bool>>`)
- [x] 6.3 Ensure mixed audio stream flows to encoder unchanged
- [ ] 6.4 Handle partial source disconnection (continue with remaining source)
- [ ] 6.5 Test A/V sync with dual audio sources

## 7. Frontend - Configuration UI

- [x] 7.1 Add microphone dropdown to Audio settings group
- [x] 7.2 Populate microphone dropdown with input devices from backend
- [x] 7.3 Add echo cancellation toggle (visible when mic selected)
- [x] 7.4 Wire microphone selection to config save
- [x] 7.5 Wire echo cancellation toggle to config save
- [x] 7.6 Handle microphone dropdown disabled state when audio disabled

## 8. Frontend - Tauri Commands

- [x] 8.1 Update `get_audio_sources` command to distinguish input vs output sources
- [x] 8.2 Create or update command to save microphone configuration
- [x] 8.3 Ensure config load returns microphone and AEC settings

## 9. Testing

- [ ] 9.1 Test dual capture with system audio + microphone on Linux
- [ ] 9.2 Test echo cancellation effectiveness with speakers + mic
- [ ] 9.3 Test single source recording still works (backward compatibility)
- [ ] 9.4 Test config persistence across app restarts
- [ ] 9.5 Test with common meeting apps (Zoom, Teams, Discord)
- [ ] 9.6 Test microphone disconnection during recording
- [ ] 9.7 Test AEC toggle on/off during different scenarios

## 10. Documentation

- [ ] 10.1 Update README with microphone and AEC feature description
