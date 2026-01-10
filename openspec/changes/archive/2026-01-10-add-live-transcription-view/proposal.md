# Change: Add Live Transcription View

## Why

Users who enable transcription currently have no way to see transcription output during recording. They must wait until recording completes and then open the markdown transcript file. A live transcription view provides immediate feedback that transcription is working and shows the text as it is generated, improving the user experience for voice-over recordings, narration, and accessibility use cases.

## What Changes

- Add a new secondary window that displays transcription output in real-time during recording
- The transcription window is tall and narrow (sidebar-style), resizable and movable
- Uses the same theme colors and border styling as the main application window
- Add a configuration checkbox "Show transcript when recording starts" (default: checked)
- When recording starts with transcription enabled and the setting is on, the transcription window automatically opens
- As transcription segments are produced, they are appended to the window with timestamps
- The window remains open after recording stops so the user can review the transcript
- The transcription window can be manually closed at any time

## Impact

- Affected specs: app-configuration (new setting), voice-transcription (no changes to transcription itself, only display)
- New spec: live-transcription-view (new capability)
- Affected code:
  - `src-tauri/` - New Tauri command to open transcription window, event emission for transcript segments
  - `src-service/` - Emit events when transcription segments are produced (currently only writes to file)
  - `src/` - New HTML/CSS/TS for transcription window, update main.ts for configuration
  - `src-tauri/tauri.conf.json` - Define transcription window configuration
