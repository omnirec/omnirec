# Design: Windows Microphone Capture with Audio Mixing and AEC

## Context

OmniRec has full dual-audio capture implemented on Linux using PipeWire. Windows currently has working single-source WASAPI capture (both loopback for system audio and direct capture for microphones), but lacks the ability to capture both simultaneously and mix them together. The existing `start_audio_capture_dual()` function falls back to single-source capture.

The Linux implementation uses:
- Two PipeWire streams (one for system audio, one for microphone)
- An `AudioMixer` struct that buffers and mixes samples
- The `aec3` crate for acoustic echo cancellation
- Frame-based processing (10ms frames for AEC compatibility)

## Goals

- Enable simultaneous capture of system audio (WASAPI loopback) and microphone (WASAPI direct)
- Mix both audio streams using the same algorithm as Linux
- Apply AEC to the microphone input using the `aec3` crate
- Maintain API compatibility with Linux implementation

## Non-Goals

- Multiple microphone support (only one mic at a time)
- Separate audio tracks per source
- Platform-specific AEC tuning (use same settings as Linux)
- Changes to the encoder or frontend

## Decisions

### 1. Threading Architecture

**Decision**: Spawn two WASAPI capture threads (one for system audio, one for microphone) that feed samples to a shared mixer on a third thread.

**Rationale**: 
- WASAPI capture requires dedicated threads with COM initialization
- The existing single-source capture already uses this pattern
- A separate mixer thread avoids blocking capture with AEC processing

**Architecture**:
```
WASAPI Loopback Thread ──────┐
  (system audio)             │
                             ├──► AudioMixer Thread ──► mpsc channel ──► Encoder
WASAPI Capture Thread ───────┘
  (microphone)
```

### 2. Sample Coordination

**Decision**: Use a channel-based approach where each capture thread sends samples to the mixer thread.

**Implementation**:
```rust
enum MixerInput {
    SystemAudio(Vec<f32>),
    Microphone(Vec<f32>),
    Stop,
}
```

The mixer thread buffers samples from both sources and processes them when sufficient data is available.

### 3. AudioMixer Port

**Decision**: Port the Linux `AudioMixer` struct with minimal changes.

**Key components to port**:
- Sample buffering (`buffer_mic`, `buffer_sys`)
- Frame-based AEC processing (480 samples/channel at 48kHz)
- Linear mixing with clipping protection
- AEC enable/disable toggle

**Differences from Linux**:
- Input via channels instead of direct PipeWire callbacks
- Runs on a dedicated thread instead of PipeWire's main loop

### 4. AEC3 Integration

**Decision**: Use the same `aec3::voip::VoipAec3` configuration as Linux.

```rust
let aec = VoipAec3::builder(48000, 2, 2)
    .enable_high_pass(true)
    .build()?;
```

The `aec3` crate is pure Rust with no platform-specific code, so it should work unchanged on Windows.

### 5. Sample Format Handling

**Decision**: Both capture threads convert to 48kHz stereo f32 before sending to mixer.

**Current state**: The existing Windows capture code already handles format conversion in `convert_samples_to_f32()`, including mono-to-stereo expansion.

### 6. Stop Signal Handling

**Decision**: Use `Arc<AtomicBool>` stop flags (same as current implementation) plus a `Stop` message to the mixer thread.

Both capture threads and the mixer thread check the stop flag and terminate gracefully.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Thread synchronization complexity | Use bounded channels with reasonable capacity |
| AEC3 cross-platform compatibility | Test early; crate is pure Rust, should work |
| Increased latency from buffering | 10ms frame size is acceptable for recording |
| CPU overhead from AEC | Can be disabled via configuration |

## Implementation Approach

The implementation reuses as much existing code as possible:

1. **Existing code to keep**:
   - `list_audio_sources()` - already enumerates both input and output devices
   - `start_audio_capture()` - single-source capture remains unchanged
   - `convert_samples_to_f32()` - format conversion
   - `run_capture_thread()` / `run_capture_loop()` - WASAPI capture logic

2. **New code to add**:
   - `AudioMixer` struct (ported from Linux with modifications)
   - `start_audio_capture_dual_impl()` - orchestrates dual capture
   - Mixer thread function
   - Channel types for inter-thread communication

3. **Code to modify**:
   - `WindowsBackend::start_audio_capture_dual()` - call new implementation

## Open Questions

None - the Linux implementation provides a proven design to follow.
