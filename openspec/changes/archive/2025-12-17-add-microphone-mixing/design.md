# Design: Microphone Input with Audio Mixing and Echo Cancellation

## Context

OmniRec currently supports capturing a single audio source (either system audio OR microphone) on Linux via PipeWire. Users recording meetings need both:
1. System audio (the meeting participants speaking)
2. Their own voice via microphone

When using a microphone near speakers, the speaker output is picked up by the mic, creating an echo effect (the meeting audio appears twice - once from system capture, once from the mic pickup). Acoustic Echo Cancellation (AEC) can remove this echo by filtering the known system audio from the microphone input.

## Goals

- Enable simultaneous capture of system audio and microphone input
- Mix both audio streams into a single stereo output track
- Apply AEC to the microphone input to remove speaker bleed
- Provide user control over AEC (enable/disable)
- Maintain existing single-source audio capture as the default behavior
- Linux-only implementation (extends existing PipeWire audio capture)

## Non-Goals

- Multiple microphone support (only one mic at a time)
- Separate audio tracks per source (single mixed track only)
- Real-time AEC quality adjustment or tuning
- Windows/macOS implementation (remains stub)
- Hardware AEC integration (software-only)

## Decisions

### 1. Audio Mixing Architecture

**Decision**: Mix audio in the capture layer before sending to encoder, outputting a single combined stream.

**Rationale**: 
- Simpler encoder path (no changes to FFmpeg command)
- Easier A/V sync (single audio timeline)
- Reduces complexity vs. multiple audio tracks

**Implementation**:
```
System Audio Stream ─────┐
                         ├──► Audio Mixer ──► Single Mixed Stream ──► Encoder
Microphone Stream ───────┘
        │
        └──► AEC Filter (uses system audio as reference)
```

### 2. Acoustic Echo Cancellation Approach

**Decision**: Use the `aec3` crate - a pure Rust port of WebRTC's AEC3 algorithm.

**Rationale**: 
- Battle-tested WebRTC AEC3 algorithm (state-of-the-art, used in Chrome)
- Pure Rust implementation - no external C dependencies
- Native support for 48kHz sample rate (matches PipeWire's native rate)
- Already proven in the flowstt project with good results
- `aec3 = "0.1"` - minimal dependency

**Reference implementation**: See `~/ws/flowstt/src-tauri/src/pipewire_audio.rs` for working AEC3 integration with PipeWire.

**Alternatives rejected**:
- PipeWire echo-cancel module: Requires system configuration, may not be available
- Speex-based AEC (aec-rs): Severe quality issues at 48kHz - designed for 8-16kHz telephony

### 3. AEC3 Integration

**Decision**: Apply AEC in the audio mixer before combining streams, using the `aec3::voip::VoipAec3` API.

**Configuration**:
```rust
use aec3::voip::VoipAec3;

// AEC3 frame size: 10ms at 48kHz = 480 samples per channel
const AEC_FRAME_SAMPLES: usize = 480;

// Initialize AEC3 for 48kHz stereo
let aec = VoipAec3::builder(48000, 2, 2)
    .enable_high_pass(true)
    .build()?;
```

**Processing flow**:
```rust
// mic_samples: microphone input (may contain echo)
// ref_samples: system audio (the reference - what to remove)
// out: cleaned microphone output
let mut out = vec![0.0f32; mic_samples.len()];
aec.process(&mic_samples, Some(&ref_samples), false, &mut out)?;
```

**Key constraints**:
- AEC3 requires exactly `frame_samples * channels` samples per frame (480 * 2 = 960 for stereo)
- Both streams must be buffered and processed in aligned frame chunks
- System audio serves as the "render" (reference) signal
- Microphone serves as the "capture" signal

### 4. Audio Mixer Design

**Decision**: Create an `AudioMixer` struct that buffers samples from both streams and processes them in AEC-compatible frame sizes.

**Implementation pattern** (from flowstt):
```rust
struct AudioMixer {
    /// Buffer for microphone samples (primary)
    buffer_mic: Vec<f32>,
    /// Buffer for system audio samples (reference)
    buffer_sys: Vec<f32>,
    /// Number of active streams (1 or 2)
    num_streams: usize,
    /// Channels per stream
    channels: u16,
    /// AEC3 instance (created when 2 streams active)
    aec: Option<VoipAec3>,
    /// Flag to enable/disable AEC (shared with config)
    aec_enabled: Arc<Mutex<bool>>,
}
```

**Buffer processing**:
1. Samples arrive from PipeWire callbacks into respective buffers
2. When both buffers have at least `AEC_FRAME_SAMPLES * channels` samples:
   - Extract one frame from each buffer
   - If AEC enabled: process mic through AEC with system audio as reference
   - Mix processed mic with system audio (0.5 gain each)
   - Send mixed output to encoder channel
3. Repeat until buffers are depleted below frame size

### 5. Sample Synchronization

**Decision**: Use frame-based buffering without explicit timestamps.

**Rationale**: PipeWire delivers samples at the same rate from both streams. Small timing differences are absorbed by the buffer. AEC3 has internal delay estimation that handles acoustic path latency.

**Buffer management**:
- Process frames only when both buffers have sufficient data
- Without AEC: process any aligned amount (channel-aligned)
- With AEC: must process in exact frame sizes (480 samples/channel)

### 6. Configuration Structure

**Decision**: Extend `AudioConfig` with microphone and AEC settings:

```rust
pub struct AudioConfig {
    /// Whether audio recording is enabled.
    pub enabled: bool,
    /// Selected system audio source ID (output monitor).
    pub source_id: Option<String>,
    /// Selected microphone source ID.
    pub microphone_id: Option<String>,
    /// Whether AEC is enabled for microphone input.
    pub echo_cancellation: bool,
}
```

**Defaults**:
- `enabled`: true
- `source_id`: None (user must select)
- `microphone_id`: None (disabled by default)
- `echo_cancellation`: true (when mic is selected)

### 7. UI Design

**Decision**: Add microphone dropdown and AEC toggle below existing audio source dropdown.

```
Audio Settings
├── Enable audio recording [toggle: on]
├── System audio source [dropdown: Speakers (Monitor)]
├── Microphone [dropdown: None / Built-in Mic / USB Mic / ...]
└── Echo cancellation [toggle: on] (only shown when mic selected)
```

**Behavior**:
- Microphone dropdown shows "None" plus all input devices
- AEC toggle only appears when microphone is selected
- Both dropdowns refresh on open (like system audio)

### 8. Mixing Algorithm

**Decision**: Simple linear mixing with clipping protection.

```rust
fn mix_samples(system: &[f32], processed_mic: &[f32]) -> Vec<f32> {
    // Both are stereo 48kHz, same length after frame alignment
    processed_mic.iter()
        .zip(system.iter())
        .map(|(&mic, &sys)| ((mic + sys) * 0.5).clamp(-1.0, 1.0))
        .collect()
}
```

**Note**: Equal 0.5 weighting preserves headroom. Could add volume controls in future.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| AEC quality varies by environment | Provide toggle to disable if problematic |
| AEC convergence time (~few seconds) | Document that initial echo suppression may be partial |
| Stream synchronization issues | Frame-based buffering absorbs minor timing differences |
| Increased CPU usage from AEC | AEC3 is optimized for real-time; can be disabled |
| Mono microphones | Convert mono to stereo by duplicating channels |
| Latency from frame buffering | 10ms frame size is minimal, acceptable for recording |

## Migration Plan

1. Add `aec3 = "0.1"` dependency to Cargo.toml
2. Create `AudioMixer` struct with dual-stream buffering
3. Integrate AEC3 processing in mixer
4. Modify PipeWire audio capture to support two simultaneous streams
5. Add configuration fields and persistence
6. Update UI with microphone selection and AEC toggle
7. Test with common meeting applications (Zoom, Teams)

No breaking changes - existing recordings with single audio source continue to work unchanged.

## Open Questions

1. Should we expose mixing volume controls? (Deferred - start with equal 0.5 weighting)
2. Should AEC be per-microphone or global? (Decision: global toggle)
3. What happens if system audio is "None" but mic is selected? (Allow mic-only recording, AEC skipped)
