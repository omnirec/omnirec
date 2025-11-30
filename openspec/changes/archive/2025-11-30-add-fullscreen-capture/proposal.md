# Change: Add Full-Screen Display Recording

## Why

Users currently can only record individual windows or custom screen regions. Full-screen recording of an entire display is a common use case (presentations, tutorials, gameplay) that requires a simpler workflow than region selection when the user wants to capture everything on a specific monitor.

## What Changes

- Add a new "Display" capture mode alongside existing "Window" and "Region" modes
- Add a display selector dropdown that lists all connected monitors with their names and resolutions
- Implement full-screen capture using the existing monitor capture infrastructure (reuse `region_recorder.rs` pattern with full monitor dimensions)
- Update the UI mode toggle to include three options: Window, Region, Display

## Impact

- Affected specs:
  - `region-capture` - Modify Capture Mode Selection requirement to include "Display" option
  - New capability: `display-capture` - Display enumeration and selection for full-screen recording
- Affected code:
  - `src/main.ts` - Add display mode UI logic and display list rendering
  - `index.html` - Add display selection section and mode button
  - `src/styles.css` - Add display list styling
  - `src-tauri/src/lib.rs` - Add new Tauri command for display capture
  - `src-tauri/src/capture/` - Potentially add display recorder (or reuse region_recorder with full dimensions)
