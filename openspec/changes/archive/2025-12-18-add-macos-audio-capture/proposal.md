# Change: Add macOS Audio Device Enumeration and System Audio Capture

## Why

macOS audio capture is currently stubbed, returning `NotImplemented` errors. This blocks recording with audio on macOS. The cross-platform porting plan identifies this as the next major milestone after completing Windows audio support. Implementing audio enumeration and system audio capture will enable macOS users to record screen content with system audio.

## What Changes

- Implement audio device enumeration on macOS using Core Audio APIs
- Implement system audio capture on macOS using ScreenCaptureKit audio
- Replace the current stub in `src-tauri/src/capture/macos/audio.rs` with working implementation
- Modify the `audio-capture` spec to remove stub requirements for macOS enumeration and capture
- **UI Change**: Replace system audio device dropdown with a checkbox on macOS (SCK captures all system audio, not per-device)
- Implement proactive permission handling with clear user guidance

**Note**: Microphone capture and dual audio mixing with AEC are deferred to subsequent changes (`add-macos-microphone-capture` and `add-macos-dual-audio-mixing`) per the cross-platform porting plan.

## Impact

- Affected specs: `audio-capture`
- Affected code:
  - `src-tauri/src/capture/macos/audio.rs` - Primary implementation file (~300-400 lines)
  - `src-tauri/Cargo.toml` - May need `coreaudio-rs` or similar dependency
- Dependencies: Requires macOS 13+ for ScreenCaptureKit audio capture
- No breaking changes to existing API

## Technical Approach

### Audio Device Enumeration

Use Core Audio `AudioObjectGetPropertyData` with:
- `kAudioHardwarePropertyDevices` to list all audio devices
- `kAudioDevicePropertyStreams` with `kAudioDevicePropertyScopeInput/Output` to determine device type
- `kAudioDevicePropertyDeviceNameCFString` for friendly names

### System Audio Capture

Use ScreenCaptureKit with audio enabled:
- Configure `SCStreamConfiguration.capturesAudio = true`
- Process `CMSampleBuffer` audio samples from stream delegate
- Resample to 48kHz stereo f32 format in capture module (not encoder)

### macOS-Specific UI

Since ScreenCaptureKit captures all system audio globally (not per-device):
- Replace system audio dropdown with "Record system audio" checkbox on macOS
- Checkbox disabled with tooltip on macOS < 13
- Backend receives boolean flag instead of device ID

### Permission Handling

- Check permission status before starting capture
- Trigger permission prompt if undetermined, return helpful error
- Return `PermissionDenied` with System Settings instructions if denied
- Screen recording permission covers both video and audio

### Minimum Version

- Audio enumeration: macOS 10.14+ (Core Audio)
- System audio capture: macOS 13+ (ScreenCaptureKit audio)
- For macOS 12.3-12.x: Return `NotImplemented` for capture (enumeration still works)
