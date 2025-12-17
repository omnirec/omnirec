# Design: Audio Recording Support

## Context

OmniRec captures screen video using platform-specific APIs (PipeWire on Linux, ScreenCaptureKit on macOS, DXGI on Windows). The video frames are piped to FFmpeg for encoding. Adding audio requires:

1. Capturing audio samples from system audio sources
2. Synchronizing audio with video frames
3. Muxing audio and video into the output MP4

This change implements audio support for **Linux only**. Windows and macOS will have stub implementations.

## Goals

- Capture system audio (desktop audio output) alongside screen video on Linux
- Allow users to enable/disable audio recording
- Allow users to select which audio source to record
- Maintain A/V sync throughout the recording
- Minimal impact on existing video-only recording path
- Stub implementations for Windows/macOS to maintain cross-platform build

## Non-Goals

- Microphone input capture (future enhancement)
- Audio-only recording
- Live audio preview/monitoring
- Audio level meters in UI
- Per-application audio capture (platform limitations)
- Windows audio capture (future change)
- macOS audio capture (future change)

## Decisions

### 1. Audio Capture Architecture

**Decision**: Use separate audio capture thread that feeds samples to FFmpeg via a second stdin pipe.

**Rationale**: FFmpeg can accept multiple inputs and mux them. Using a separate audio thread avoids blocking the video capture pipeline. The `-itsoffset` flag or timestamps can handle minor timing differences.

**Alternatives considered**:
- Single-threaded capture: Would block video on audio or vice versa
- Post-recording mux: Would require temp files and add complexity
- External audio process: Would complicate IPC and state management

### 2. Platform Implementation Strategy

**Decision**: Implement Linux fully, stub Windows/macOS.

| Platform | Implementation | Status |
|----------|----------------|--------|
| Linux | PipeWire audio capture | Full |
| Windows | Return `NotImplemented` | Stub |
| macOS | Return `NotImplemented` | Stub |

**Rationale**: Linux is the primary development platform and PipeWire is already used for video capture. This allows shipping audio support quickly while maintaining cross-platform builds.

### 3. Linux Audio API (PipeWire)

**Decision**: Use PipeWire for audio capture on Linux.

**Rationale**: PipeWire is already used for video capture. It provides:
- Direct access to audio sinks (system audio output)
- Low-latency capture
- Integration with the existing capture infrastructure

**Implementation approach**:
- Query PipeWire for audio sink nodes
- Create a stream to capture from the selected sink
- Feed PCM samples to the encoder

### 4. Audio Source Enumeration

**Decision**: Enumerate available audio output devices and let users select one, defaulting to none (no audio).

**Rationale**: Users may have multiple audio outputs (speakers, headphones, HDMI audio). Letting them choose ensures they capture the right audio. Defaulting to none prevents unexpected audio capture for users who only want video.

### 5. FFmpeg Audio Integration

**Decision**: Modify the FFmpeg command to accept audio via a named pipe (FIFO) or additional stdin with PCM samples.

Current video-only command:
```
ffmpeg -f rawvideo -pix_fmt bgra -s WxH -r 30 -i - -c:v libx264 ... output.mp4
```

With audio:
```
ffmpeg -f rawvideo -pix_fmt bgra -s WxH -r 30 -i pipe:0 \
       -f s16le -ar 48000 -ac 2 -i pipe:3 \
       -c:v libx264 -c:a aac ... output.mp4
```

**Rationale**: FFmpeg handles A/V muxing and timestamp alignment. Using pipes avoids temp files.

### 6. Audio Format

**Decision**: Capture at 48kHz stereo (2 channels), 16-bit signed integer PCM. Encode to AAC in the output MP4.

**Rationale**: 48kHz is the standard for video production. AAC is universally supported in MP4 containers. 16-bit PCM is simple to handle and sufficient quality.

### 7. Configuration Storage

**Decision**: Add audio settings to the existing `AppConfig` struct:

```rust
pub struct AudioConfig {
    pub enabled: bool,           // Default: true
    pub source_id: Option<String>, // Default: None (no source selected)
}
```

**Rationale**: Follows existing configuration patterns. Persists to the same config file.

### 8. UI Integration

**Decision**: Add an "Audio" settings group in the configuration view with:
- Toggle switch for enable/disable (default: enabled)
- Dropdown for audio source selection (default: "None")

**Rationale**: Keeps audio settings with other output settings. Simple, non-intrusive UI.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| A/V sync drift over long recordings | Use timestamps and let FFmpeg handle alignment; test with 30+ minute recordings |
| Audio source disappears during recording | Detect disconnection, continue recording video-only, notify user |
| No audio sources available | Show "No audio sources found" message; allow video-only recording |
| PipeWire version compatibility | Test on common distros (Arch, Ubuntu 22.04+, Fedora) |

## Migration Plan

1. Implement audio capture trait and Linux implementation
2. Add stub implementations for Windows/macOS
3. Add audio configuration and UI
4. Modify encoder to support audio muxing
5. Test on Linux

No breaking changes - existing video-only recordings continue to work. Users must explicitly select an audio source to enable audio recording.

## Open Questions

1. Should we show audio level indication in the UI? (Deferred - adds complexity)
2. Should audio settings be per-session or persistent? (Decision: persistent via config file)
3. Should we support multiple audio sources? (Deferred - start with single source)

## Future Work

- Windows audio capture via WASAPI loopback
- macOS audio capture via ScreenCaptureKit
- Microphone input support
- Audio level meters
