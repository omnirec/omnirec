# Design: macOS Audio Device Enumeration and System Audio Capture

## Context

OmniRec has full audio support on Linux (PipeWire) and Windows (WASAPI), but macOS audio is stubbed. The existing video capture on macOS uses ScreenCaptureKit, which also supports audio capture starting in macOS 13.

### Stakeholders

- macOS users who want to record with system audio
- Developers maintaining cross-platform parity

### Constraints

- Must use native macOS APIs (no third-party audio routing software like Soundflower/BlackHole)
- Must integrate with existing `AudioSource`, `AudioSample`, and `StopHandle` types
- Must handle permissions gracefully (screen recording permission covers audio)
- ScreenCaptureKit audio requires macOS 13+

## Goals / Non-Goals

### Goals

- Enumerate output and input audio devices on macOS
- Capture system audio (loopback) from output devices
- Match existing audio sample format (48kHz stereo f32)
- Handle device hot-plug during enumeration
- Provide clear error messages for permission issues

### Non-Goals

- Microphone capture (separate change: `add-macos-microphone-capture`)
- Dual audio mixing with AEC (separate change: `add-macos-dual-audio-mixing`)
- Support for macOS versions before 10.14 (enumeration) or 13 (capture)
- Virtual audio device creation

## Decisions

### Decision 1: Use Core Audio for Device Enumeration

**Choice**: Use Core Audio (`AudioHardware*` APIs) rather than AVFoundation for device enumeration.

**Rationale**:
- Core Audio provides lower-level access to all audio devices
- AVFoundation `AVCaptureDevice` doesn't expose output devices (only inputs)
- Core Audio is available since macOS 10.0
- Consistent with professional audio applications

**Alternatives Considered**:
- AVFoundation: Only exposes input devices, not suitable for output/loopback enumeration
- `coreaudio-rs` crate: Higher-level abstraction, but may limit access to specific device properties

### Decision 2: Use ScreenCaptureKit for System Audio Capture

**Choice**: Use ScreenCaptureKit's `capturesAudio` feature rather than a loopback approach.

**Rationale**:
- ScreenCaptureKit is already used for video capture
- Built-in audio capture without needing virtual audio devices
- Apple's recommended approach for screen recording with audio
- Single permission prompt covers both video and audio

**Alternatives Considered**:
- Core Audio tap on output device: Requires additional permissions and complexity
- Virtual audio device (Soundflower/BlackHole): Requires user to install third-party software
- AVCaptureSession: Would require separate video+audio capture coordination

### Decision 3: Reuse Existing ScreenCaptureKit Stream

**Choice**: Extend the existing macOS capture module to enable audio on the existing SCStream.

**Rationale**:
- Avoids creating a separate audio-only stream
- Leverages existing permission handling and stream lifecycle
- Audio samples are synchronized with video by the framework

**Implementation**: Modify `start_display_capture` and `start_window_capture` to optionally enable audio, or create a new audio-specific capture function.

### Decision 5: Resample in Capture Module

**Choice**: Perform resampling to 48kHz stereo in the macOS audio capture module, not the encoder.

**Rationale**:
- Consistency with Linux (PipeWire) and Windows (WASAPI) implementations
- Encoder receives uniform format from all platforms
- Simplifies encoder code

### Decision 6: Checkbox UI for macOS System Audio

**Choice**: On macOS, replace the system audio device dropdown with a simple "Record system audio" checkbox.

**Rationale**:
- ScreenCaptureKit captures all system audio globally; per-device selection is not supported
- A checkbox accurately reflects the actual capability
- Simpler UX that doesn't mislead users into thinking they can select specific output devices

**Implementation**:
- Frontend detects macOS platform and renders checkbox instead of dropdown
- Backend receives boolean `enable_system_audio` flag instead of device ID for macOS
- Device enumeration for output devices is still implemented (useful for future microphone selection)

### Decision 7: Graceful Permission Handling

**Choice**: Handle permission requests proactively with clear user guidance.

**Rationale**:
- macOS requires explicit screen recording permission for SCK audio
- Users should understand what permission is needed and why before hitting errors
- Poor permission UX leads to support burden

**Implementation**:
- Check permission status before starting capture
- If undetermined: trigger permission prompt, return `PermissionDenied` with helpful message
- If denied: return `PermissionDenied` with instructions to enable in System Settings
- UI should display permission status and provide guidance

### Decision 4: Runtime Version Check for Audio Capture

**Choice**: Check macOS version at runtime and return `NotImplemented` for audio capture on macOS < 13.

**Rationale**:
- Allows the app to run on macOS 12.3+ (for video)
- Graceful degradation: users on older macOS can still record video without audio
- Avoids compile-time feature flags that complicate deployment

## Architecture

```
                        +------------------+
                        |   Frontend UI    |
                        +--------+---------+
                                 |
                                 v
                        +--------+---------+
                        |  Tauri Commands  |
                        +--------+---------+
                                 |
             +-------------------+-------------------+
             |                                       |
             v                                       v
    +--------+---------+                   +---------+--------+
    | list_audio_sources|                  | start_audio_capture|
    +--------+---------+                   +---------+--------+
             |                                       |
             v                                       v
    +--------+---------+                   +---------+--------+
    |   Core Audio     |                   | ScreenCaptureKit |
    | (Enumeration)    |                   | (Audio Capture)  |
    +------------------+                   +------------------+
```

### Data Flow for Audio Capture

1. Frontend calls `start_audio_capture(source_id)`
2. Backend identifies source as output device (system audio)
3. Backend creates/configures SCStream with `capturesAudio = true`
4. SCStream delegate receives `CMSampleBuffer` with audio
5. Samples are converted to f32, resampled to 48kHz if needed
6. Samples sent through `mpsc::Sender<AudioSample>` channel
7. Encoder receives samples for muxing with video

## Risks / Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| ScreenCaptureKit audio only on macOS 13+ | Users on macOS 12 cannot record audio | Clear error message; document requirement |
| Core Audio API complexity | Implementation errors | Use `objc2` crate for safe bindings; thorough testing |
| Audio/video sync issues | A/V drift in recordings | SCK provides synchronized samples; verify in testing |
| Device hot-plug during capture | Capture failure | Handle `kAudioHardwarePropertyDevices` changes |

## Migration Plan

1. Implement device enumeration first (works on macOS 10.14+)
2. Test enumeration independently
3. Implement audio capture (requires macOS 13+)
4. Update spec to reflect actual behavior
5. No data migration needed (new feature)

## Rollback

If issues are discovered post-release:
- Revert to stub implementation by reverting the code changes
- Spec can remain as-is (stub behavior is already documented)

## Resolved Questions

1. **Sample rate handling**: Resample to 48kHz in the capture module for consistency with Linux/Windows. (Decision 5)

2. **Device selection for system audio**: Use a checkbox UI instead of device dropdown since SCK captures all system audio. (Decision 6)

3. **Permissions for audio-only capture**: Handle proactively with clear guidance. (Decision 7)
   - To verify during implementation: Does Core Audio enumeration work without permission?
