# Change: Add Region Capture Mode

## Why

Users need to record specific areas of their screen rather than entire windows. This is common for:
- Recording a portion of a large application
- Capturing content that spans multiple windows
- Creating focused tutorial content without distractions
- Recording from applications that don't have distinct window handles

## What Changes

- **NEW** Region capture mode alongside existing window capture
- **NEW** Display enumeration to support multi-monitor setups
- **NEW** Interactive region selection overlay with drag-to-select functionality
- **NEW** Resizable and repositionable selection rectangle
- **MODIFIED** Recording controls to support both capture modes (window vs region)
- **MODIFIED** Capture backend to support screen/region capture in addition to window capture

## Impact

- **Affected specs:**
  - `region-capture` (new capability)
  - `recording-control` (minor modification to support capture mode selection)
  
- **Affected code:**
  - `src-tauri/src/capture/` - Add region capture module, display enumeration
  - `src-tauri/src/lib.rs` - New Tauri commands for region capture
  - `src-tauri/src/state.rs` - Support for region-based recording
  - `src/main.ts` - UI for mode switching and region selection
  - `src/styles.css` - Styling for selection overlay
  - New overlay window for region selection

## Assumptions

1. Region selection constrained to single monitor (simplifies capture implementation)
2. Minimum region size of 100x100 pixels to ensure usable recordings
3. Selection overlay disappears during recording (no persistent highlight)
4. Selection persists until user selects a new region or switches to window mode
