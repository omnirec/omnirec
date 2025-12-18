# Design: Windows Audio Capture via WASAPI

## Context

OmniRec needs to capture system audio on Windows to match Linux functionality. Windows provides the Windows Audio Session API (WASAPI) for low-latency audio capture. Audio device enumeration is already implemented using WASAPI; this change extends it to actual capture.

### Stakeholders
- Windows users wanting to record screen with audio
- Existing Linux implementation serves as reference

### Constraints
- Must capture 48kHz stereo audio to match encoder expectations
- Must handle devices that provide different native formats
- Must support both loopback (system audio) and direct (microphone) capture
- Must handle device disconnection gracefully

## Goals / Non-Goals

### Goals
- Implement system audio capture via WASAPI loopback on output devices
- Implement microphone capture via WASAPI on input devices
- Match the audio format contract: 48kHz stereo f32 samples
- Support capture start/stop via the existing `StopHandle` mechanism

### Non-Goals
- Dual audio mixing with AEC (future change: `add-windows-dual-audio-mixing`)
- Audio device hot-plugging notification (enumeration refresh is sufficient)
- Sample rate conversion for non-48kHz devices (will require later enhancement if needed)

## Decisions

### Decision: Use WASAPI in Shared Mode with Loopback Flag

**What**: Use `IAudioClient::Initialize` with `AUDCLNT_SHAREMODE_SHARED` and `AUDCLNT_STREAMFLAGS_LOOPBACK` for system audio capture.

**Why**: 
- Loopback mode captures what's being played through speakers without requiring a virtual audio device
- Shared mode allows other applications to continue using audio
- This is the standard approach for screen recording applications on Windows

**Alternatives considered**:
- Exclusive mode: Would block other audio applications - rejected
- Virtual audio device (like VB-Audio): Requires user installation - rejected
- DirectSound: Deprecated, doesn't support loopback - rejected

### Decision: Capture Thread Architecture

**What**: Spawn a dedicated thread that runs the WASAPI event-driven capture loop, sending samples through an mpsc channel.

**Why**:
- WASAPI requires periodic calls to `GetBuffer`/`ReleaseBuffer` in a tight loop
- Async/await would add unnecessary complexity for this CPU-bound work
- Matches the Linux PipeWire implementation's threading model

### Decision: Format Handling

**What**: Request the device's native format via `GetMixFormat`, then:
1. If stereo 48kHz float32 - use directly
2. If different format - convert samples to 48kHz stereo float32

**Why**:
- WASAPI shared mode requires using the device's mix format
- The encoder expects a consistent format regardless of device capabilities
- Most Windows audio devices default to 48kHz stereo, minimizing conversion overhead

### Decision: Use `windows` Crate Directly

**What**: Use the `windows` crate for WASAPI bindings rather than a higher-level audio crate.

**Why**:
- Already used for enumeration - maintains consistency
- Direct control over loopback capture flags
- Avoids additional dependencies
- Higher-level crates (cpal, wasapi) may not expose all needed WASAPI features

## Risks / Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| Device format mismatch | Medium | Implement format conversion; log warning if conversion needed |
| Thread synchronization complexity | Low | Use simple mpsc channel pattern proven in Linux implementation |
| COM apartment threading issues | Medium | Initialize COM as MTA per-thread; document threading requirements |
| Loopback captures silence when nothing playing | Low | Expected behavior; document in user guide |

## Technical Details

### WASAPI Capture Flow

```
1. CoInitializeEx(MTA)
2. Get IMMDevice from endpoint ID
3. Activate IAudioClient
4. GetMixFormat() for native format
5. Initialize with LOOPBACK flag (for output devices) or without (for input)
6. GetService<IAudioCaptureClient>
7. Start()
8. Loop:
   a. GetBuffer() - get available samples
   b. Convert format if needed
   c. Send to channel
   d. ReleaseBuffer()
   e. Check stop flag
9. Stop() and cleanup
```

### Sample Format Conversion

Input formats to handle:
- Float32 stereo (ideal - pass through)
- Float32 mono (duplicate to stereo)
- Int16 stereo (convert to float32)
- Int16 mono (convert to float32, duplicate to stereo)
- Different sample rates (resample to 48kHz)

### Error Handling

- Device disconnection: Return gracefully, close channel
- COM errors: Log and return CaptureError::AudioError
- Buffer overruns: Log warning, continue capture

## Open Questions

1. **Sample rate conversion**: Should we implement resampling for non-48kHz devices, or require 48kHz? 
   - Initial implementation: Accept only 48kHz devices, return error for others
   - Future: Add resampling support if needed

2. **Buffer size tuning**: What buffer duration provides good latency without dropouts?
   - Initial: Use WASAPI default (~10ms typical)
   - Can tune based on testing
