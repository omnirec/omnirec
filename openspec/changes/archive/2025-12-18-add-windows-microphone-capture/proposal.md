# Change: Add Windows Microphone Capture with Audio Mixing and AEC

## Why

Windows users cannot record meetings with both system audio and their own voice via microphone. While single-source capture works (system audio OR microphone), there is no support for capturing both simultaneously and mixing them into a single audio track. This prevents Windows users from creating complete meeting recordings with their own commentary. This feature already exists on Linux using PipeWire and the aec3 crate.

## What Changes

- Implement dual audio source capture on Windows (system audio + microphone simultaneously)
- Port the audio mixing algorithm from Linux to work with WASAPI capture threads
- Integrate the aec3 crate for acoustic echo cancellation on Windows
- Update `WindowsBackend::start_audio_capture_dual()` to support full dual-source capture instead of falling back to single-source

## Impact

- Affected specs:
  - `audio-capture` - Add Windows dual audio capture requirements
- Affected code:
  - `src-tauri/src/capture/windows/audio.rs` - Add mixer, dual-stream coordination, AEC integration
  - `src-tauri/src/capture/windows/mod.rs` - Update `start_audio_capture_dual()` implementation
- Platform scope: Windows only
- Dependencies: `aec3` crate (already in use on Linux)
