# Change: Add Microphone Input with Audio Mixing and Echo Cancellation

## Why

Users need to record meetings (Zoom, Teams, etc.) with both system audio (the meeting participants) and their own voice via microphone. Currently, OmniRec only supports capturing a single audio source (system audio OR microphone), which makes it impossible to record a complete meeting experience. Additionally, when a microphone picks up speaker output, it creates an echo effect in the recording that degrades quality.

## What Changes

- Add support for selecting a secondary audio source (microphone) in addition to the primary audio source (system audio)
- Mix the two audio streams together into a single stereo track for the output MP4
- Add optional Acoustic Echo Cancellation (AEC) for the microphone input to prevent speaker output picked up by the mic from causing echo in the recording
- Add configuration option to enable/disable AEC (default: enabled)
- UI updates to allow selecting microphone source and toggling AEC

## Impact

- Affected specs:
  - `audio-capture` - Add multi-source capture and mixing requirements
  - `app-configuration` - Add microphone and AEC configuration settings
- Affected code:
  - `src-tauri/src/capture/linux/audio.rs` - Multi-stream capture and mixing logic
  - `src-tauri/src/encoder/mod.rs` - Accept mixed audio stream
  - `src-tauri/src/config.rs` - New configuration fields
  - `src/main.ts` - UI for microphone selection and AEC toggle
- Platform scope: Linux only (extends existing Linux audio capture)
