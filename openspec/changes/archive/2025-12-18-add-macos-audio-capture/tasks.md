# Tasks: Add macOS Audio Device Enumeration and System Audio Capture

## 1. Setup and Dependencies

- [x] 1.1 Add `coreaudio-sys` or equivalent Core Audio bindings to `Cargo.toml` (macOS-only)
- [x] 1.2 Verify existing `screencapturekit` crate supports audio capture configuration

## 2. Audio Device Enumeration

- [x] 2.1 Implement `list_audio_sources()` in `src-tauri/src/capture/macos/audio.rs`
  - Use Core Audio `AudioObjectGetPropertyData` with `kAudioHardwarePropertyDevices`
  - Filter devices by input/output capability
  - Extract device names using `kAudioDevicePropertyDeviceNameCFString`
  - Return `Vec<AudioSource>` with appropriate `AudioSourceType`
- [x] 2.2 Handle empty device list gracefully (no audio hardware)
- [x] 2.3 Test whether Core Audio enumeration requires screen recording permission
- [x] 2.4 Add unit test for device enumeration

## 3. System Audio Capture Implementation

- [x] 3.1 Implement `start_system_audio_capture()` in `src-tauri/src/capture/macos/audio.rs`
  - Create SCStream with `capturesAudio = true` configuration
  - Implement stream delegate to receive `CMSampleBuffer` audio samples
  - Convert `CMSampleBuffer` to `Vec<f32>` samples
- [x] 3.2 Implement resampling to 48kHz stereo in capture module
  - Use linear interpolation resampler (consistent with Windows implementation)
  - Handle mono-to-stereo conversion if needed
- [x] 3.3 Send samples through `mpsc::Sender<AudioSample>`
- [x] 3.4 Implement `StopHandle` to stop audio capture stream

## 4. Permission Handling

- [x] 4.1 Implement permission status check before starting capture
  - Use `CGPreflightScreenCaptureAccess()` or equivalent
- [x] 4.2 Handle undetermined permission state
  - Trigger permission prompt via `CGRequestScreenCaptureAccess()`
  - Return `PermissionDenied` with helpful message explaining permission was requested
- [x] 4.3 Handle denied permission state
  - Return `PermissionDenied` with instructions to enable in System Settings
- [x] 4.4 Add runtime version check for macOS 13+
  - Return `NotImplemented` with clear message on older versions

## 5. Audio Backend Initialization

- [x] 5.1 Implement `init_audio_backend()` to perform any one-time setup
  - Verify Core Audio is accessible
  - Log available audio devices for debugging

## 6. Frontend UI Changes

- [x] 6.1 Add platform detection for macOS in frontend
- [x] 6.2 Replace system audio dropdown with checkbox on macOS
  - Label: "Record system audio"
  - Default: unchecked
- [x] 6.3 Disable checkbox with tooltip on macOS < 13
- [x] 6.4 Update Tauri command to accept boolean flag for macOS system audio

## 7. Backend API Adjustments

- [x] 7.1 Add Tauri command variant for macOS system audio (boolean flag)
- [x] 7.2 Ensure existing device-based API still works for Linux/Windows

## 8. Testing

- [ ] 8.1 Manual test: Verify device enumeration returns expected devices
- [ ] 8.2 Manual test: Verify checkbox UI appears on macOS
- [ ] 8.3 Manual test: Verify permission prompt is triggered correctly
- [ ] 8.4 Manual test: Verify audio capture produces valid samples
- [ ] 8.5 Manual test: Verify audio is correctly muxed with video recording
- [ ] 8.6 Test on macOS 12 to verify graceful degradation (checkbox disabled)
- [ ] 8.7 Test permission denied scenario shows helpful message

## 9. Documentation

- [x] 9.1 Update README with macOS 13+ requirement for audio recording
- [x] 9.2 Add code comments documenting Core Audio and SCK usage patterns

## Task Dependencies

```
1.x (Setup)
 ├── 2.x (Enumeration) ──┬── 5.1 (Init)
 │                       │
 └── 3.x (Capture) ──────┼── 4.x (Permissions)
                         │
6.x (Frontend) ──────────┴── 7.x (Backend API)
                         │
                         └── 8.x (Testing)
                              │
                              └── 9.x (Docs)
```

- Tasks 2.x and 3.x can be done in parallel after 1.x
- Task 4.x depends on 3.x (permission handling is part of capture flow)
- Tasks 6.x and 7.x can be done in parallel, both depend on 2.x/3.x completion
- All testing (8.x) depends on implementation tasks
