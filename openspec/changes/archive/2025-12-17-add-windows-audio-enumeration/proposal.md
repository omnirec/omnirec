# Change: Add Windows Audio Device Enumeration

## Why

Audio recording is fully implemented on Linux via PipeWire but currently returns `NotImplemented` errors on Windows. Users on Windows cannot see available audio devices in the UI, blocking audio recording functionality. This change implements audio device enumeration as the foundation for full Windows audio capture support.

## What Changes

- Implement `list_audio_sources()` in `src-tauri/src/capture/windows/audio.rs` using WASAPI
- Enumerate both audio output devices (for system audio loopback) and input devices (microphones)
- Return `AudioSource` structs with device ID, display name, and source type consistent with Linux implementation
- Update spec to document Windows audio enumeration behavior (remove stub status)

## Impact

- Affected specs: `audio-capture`
- Affected code: `src-tauri/src/capture/windows/audio.rs`
- Dependencies: `windows` crate (already a dependency)
- Estimated effort: ~100-150 lines
- Part of Phase 3 of Cross-Platform Porting Plan (Audio Foundation)
