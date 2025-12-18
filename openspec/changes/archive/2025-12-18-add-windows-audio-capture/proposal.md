# Change: Implement Windows System Audio Capture via WASAPI Loopback

## Why

Windows audio capture currently returns `NotImplemented`, preventing users from recording system audio on Windows. The porting plan identifies this as Phase 4, item #6, with ~300 lines of estimated effort. Audio device enumeration is already implemented, so this change builds on that foundation to enable actual audio capture.

## What Changes

- Implement `start_audio_capture()` in `src-tauri/src/capture/windows/audio.rs` using WASAPI loopback capture
- Add WASAPI loopback capture for output devices (system audio)
- Add WASAPI direct capture for input devices (microphones)
- Capture audio as 48kHz stereo 16-bit PCM, converting to f32 samples for the encoder
- Handle device disconnection gracefully during recording
- Update spec to reflect Windows audio capture is now functional (remove stub requirement)

## Impact

- Affected specs: `audio-capture`
- Affected code: `src-tauri/src/capture/windows/audio.rs`, `src-tauri/src/capture/windows/mod.rs`
- Users on Windows will be able to record system audio and microphone input
- No breaking changes - this fills in a stub implementation
