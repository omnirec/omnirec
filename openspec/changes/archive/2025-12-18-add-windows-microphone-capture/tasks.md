# Tasks: Add Windows Microphone Capture

## 1. Core Mixer Implementation

- [x] 1.1 Add `AudioMixer` struct to `windows/audio.rs` with three-buffer design (capture, render, render_mix)
- [x] 1.2 Implement `StreamSamples` struct with `is_loopback` flag for AEC routing
- [x] 1.3 Port mixing algorithm with soft clipping (tanh-style saturation)
- [x] 1.4 Integrate AEC3 with proper two-phase processing:
  - `handle_render_frame()` - feed system audio IMMEDIATELY when it arrives
  - `process_capture_frame()` - process mic after render is fed
- [x] 1.5 Add `Resampler` struct for sample rate conversion to 48kHz

## 2. Dual Capture Coordination

- [x] 2.1 Create `start_audio_capture_dual()` function
- [x] 2.2 Spawn two WASAPI capture threads (loopback + direct)
- [x] 2.3 Spawn mixer thread that routes by `is_loopback` flag (not stream index)
- [x] 2.4 Wire up stop flag propagation to all threads
- [x] 2.5 Update capture threads to include `is_loopback` in StreamSamples
- [x] 2.6 Add resampling support in capture threads (device rate -> 48kHz)

## 3. Testing

- [x] 3.1 Add unit tests for `AudioMixer` mixing logic
- [x] 3.2 Add unit tests for AEC frame processing
- [x] 3.3 Add integration test for dual capture with real devices
- [x] 3.4 Manual test: Record with system audio + microphone
- [x] 3.5 Manual test: Verify AEC reduces echo when speaker output is audible to mic

## 4. Validation

- [x] 4.1 Run `cargo clippy` and fix warnings
- [x] 4.2 Run `cargo test` in src-tauri directory (35 tests pass)
- [x] 4.3 Build and test recording workflow end-to-end
