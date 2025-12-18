# Tasks: Windows Audio Capture Implementation

## 1. Core WASAPI Capture Implementation

- [x] 1.1 Add WASAPI capture structs and state management to `windows/audio.rs`
- [x] 1.2 Implement `start_audio_capture()` for output devices (loopback mode)
- [x] 1.3 Implement `start_audio_capture()` for input devices (direct capture)
- [x] 1.4 Add format detection and conversion (stereo, mono-to-stereo, int16-to-float32)
- [x] 1.5 Implement capture thread with event-driven WASAPI loop
- [x] 1.6 Handle stop flag and graceful shutdown

## 2. Error Handling and Edge Cases

- [x] 2.1 Handle device disconnection during capture
- [x] 2.2 Add proper COM initialization per capture thread
- [x] 2.3 Handle unsupported sample rates with clear error message
- [x] 2.4 Add logging for capture state transitions

## 3. Integration

- [x] 3.1 Update `WindowsBackend::start_audio_capture_dual()` to use new capture
- [x] 3.2 Verify integration with encoder pipeline

## 4. Testing

- [x] 4.1 Add unit tests for format conversion functions
- [x] 4.2 Add integration test that captures and verifies audio samples
- [ ] 4.3 Manual test: record screen with system audio on Windows
- [ ] 4.4 Manual test: record screen with microphone on Windows

## 5. Documentation

- [x] 5.1 Update code documentation with WASAPI usage notes
- [x] 5.2 Update README if Windows audio support was previously noted as unavailable

## Bug Fixes Applied

- Fixed missing `AUDCLNT_STREAMFLAGS_EVENTCALLBACK` flag that caused event handle error
- Changed from requesting specific format to using device's native mix format for WASAPI shared mode compatibility
