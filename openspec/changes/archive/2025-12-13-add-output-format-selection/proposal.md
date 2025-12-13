# Change: Add Output Format Selection

## Why

Users need the flexibility to export recordings in different formats for various use cases: MP4 for general playback, WebM for web sharing, GIF/APNG/WebP for social media and documentation, MKV for archival, and QuickTime for Apple ecosystem compatibility.

## What Changes

- Add a dropdown control in the UI to select the output format before recording
- Supported formats: MP4 (default), WebM, MKV, QuickTime (.mov), GIF, Animated PNG, Animated WebP
- All recordings are captured in high-quality MP4 (H.264) first
- If the selected format is not MP4, the system transcodes to the target format after recording completes
- The original high-quality MP4 is retained alongside the transcoded output
- Transcoding happens during the "Saving" phase with progress feedback

## Impact

- Affected specs: `recording-control` (modified)
- New capability: `output-format` (new spec)
- Affected code:
  - `src-tauri/src/encoder/mod.rs` - Add transcoding logic
  - `src/main.ts` - Add format dropdown UI
  - `src/styles.css` - Style the format dropdown
  - `index.html` - Add format dropdown element
