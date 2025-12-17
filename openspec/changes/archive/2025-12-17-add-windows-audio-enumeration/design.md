# Design: Windows Audio Device Enumeration

## Context

OmniRec needs to enumerate audio devices on Windows to enable audio recording. The Linux implementation uses PipeWire's registry to discover input devices (microphones) and output devices (sink monitors for system audio). Windows requires a different approach using WASAPI (Windows Audio Session API).

## Goals

- Enumerate all active audio playback (render) and capture devices using WASAPI
- Return results compatible with existing `AudioSource` type
- Support device name display in UI
- Provide unique device IDs for later capture operations
- No COM initialization complexity exposed to callers

## Non-Goals

- Actual audio capture (separate change: `add-windows-audio-capture`)
- Device hot-plug detection during runtime (initial enumeration only)
- Default device selection logic (handled by UI)

## Decisions

### Decision: Use WASAPI via `windows` crate

**Rationale**: The `windows` crate is already a project dependency and provides safe bindings to Windows APIs. WASAPI is the standard Windows audio API and provides:
- Device enumeration via `IMMDeviceEnumerator`
- Access to both playback (for loopback capture) and recording devices
- Device properties (friendly name, ID)

**Alternatives considered**:
- `cpal` crate: Higher-level cross-platform audio, but adds dependency and doesn't expose device IDs needed for WASAPI loopback
- `wasapi` crate: Specialized for WASAPI but overlaps with existing `windows` crate

### Decision: Map device types to AudioSourceType

**Mapping**:
- `eRender` (playback devices) -> `AudioSourceType::Output` (for system audio loopback)
- `eCapture` (recording devices) -> `AudioSourceType::Input` (microphones)

This matches the Linux convention where output devices represent system audio monitors.

### Decision: Use device endpoint ID as identifier

The WASAPI endpoint ID (from `IMMDevice::GetId()`) uniquely identifies a device and persists across sessions. This will be stored in `AudioSource.id` for use in later capture operations.

### Decision: Initialize COM per-call

Each call to `list_audio_sources()` will initialize COM with `CoInitializeEx` (MTA) and uninitialize on exit. This is safe and avoids requiring callers to manage COM lifetime.

## Implementation Approach

```
list_audio_sources()
├── CoInitializeEx (MTA)
├── CoCreateInstance(IMMDeviceEnumerator)
├── EnumAudioEndpoints(eRender, ACTIVE) -> output devices
│   └── For each device: extract ID, friendly name -> AudioSource(Output)
├── EnumAudioEndpoints(eCapture, ACTIVE) -> input devices
│   └── For each device: extract ID, friendly name -> AudioSource(Input)
├── CoUninitialize
└── Return Vec<AudioSource>
```

## Risks / Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| COM initialization failure | High | Return empty list with logged warning |
| Device name unavailable | Low | Fall back to "Unknown Device" |
| Disabled devices not shown | Low | Intentional - only enumerate DEVICE_STATE_ACTIVE |

## Open Questions

None - this is a straightforward WASAPI enumeration pattern.
