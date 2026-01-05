# Change: Add Voice Transcription Capability

## Why

Users recording presentations, meetings, tutorials, or commentary often need a text transcript alongside the video. Currently, they must use separate transcription tools after recording. Adding real-time voice transcription during recording provides immediate value and keeps all artifacts together.

## What Changes

- **New capability**: Voice transcription using whisper.cpp with optional CUDA acceleration
- **Service integration**: Transcription runs in the background service, parallel to video/audio encoding
- **Audio pipeline**: Voice detection and word-break detection from FlowSTT, tuned for longer segments
- **UI additions**:
  - New "Transcribe voice" checkbox in Settings (Audio group)
  - Same checkbox in main UI on the Record button row (right-aligned), visible only when system audio is enabled
- **Output**: Timestamped markdown transcript file saved alongside the video
- **Build system**: whisper.cpp integration with compile-time CUDA feature flag

## Impact

- **Affected specs**: 
  - `app-configuration` (new transcription setting)
  - New `voice-transcription` spec
- **Affected code**:
  - `src-service/` - transcription module, audio pipeline integration
  - `src-tauri/` - IPC commands for transcription settings, UI state
  - `src/` - UI for transcription toggle
  - `src-common/` - shared types for transcription state
  - Build system (`Cargo.toml`, `build.rs`) - whisper.cpp dependency
- **Dependencies**:
  - whisper.cpp (via libloading FFI, like FlowSTT)
  - Whisper medium.en model (~1.5GB)
  - Optional: CUDA toolkit for GPU acceleration (compile-time feature)
