# Change: Add System Audio Recording Support

## Why

Users frequently need to capture system audio alongside screen recordings for tutorials, gameplay, presentations, and content creation. Currently OmniRec only captures video, requiring users to record audio separately and manually sync it with the video - a tedious and error-prone workflow. Adding system audio capture makes OmniRec a complete screen recording solution.

## What Changes

- **NEW**: Audio capture capability with Linux implementation (Windows/macOS stubbed for later)
  - Linux: PipeWire audio capture (integrated with existing screen capture)
  - Windows: Stub returning `NotImplemented` (future: WASAPI loopback)
  - macOS: Stub returning `NotImplemented` (future: ScreenCaptureKit)
- **MODIFIED**: Recording control to optionally include audio track in output
- **MODIFIED**: FFmpeg encoding pipeline to mux audio with video
- **MODIFIED**: App configuration to add audio settings:
  - Enable/disable audio recording (default: enabled)
  - Audio source selection dropdown (default: none/no audio selected)
- **MODIFIED**: UI to display audio configuration controls

## Impact

- Affected specs:
  - `recording-control` - Audio integration with recording workflow
  - `app-configuration` - New audio settings group
  - NEW `audio-capture` - Audio capture with Linux implementation

- Affected code:
  - `src-tauri/src/capture/linux/audio.rs` - PipeWire audio capture (new)
  - `src-tauri/src/capture/windows/audio.rs` - Stub (new)
  - `src-tauri/src/capture/macos/audio.rs` - Stub (new)
  - `src-tauri/src/encoder/mod.rs` - FFmpeg audio muxing
  - `src-tauri/src/config.rs` - Audio configuration
  - `src-tauri/src/state.rs` - Audio state management
  - `src/main.ts` - Audio UI controls
  - `src/styles.css` - Audio control styling

- Platform dependencies:
  - Linux: PipeWire (already used for video)

## Scope

This change implements full audio support for **Linux only**. Windows and macOS will have stub implementations that return `NotImplemented`, with full support added in future changes.
