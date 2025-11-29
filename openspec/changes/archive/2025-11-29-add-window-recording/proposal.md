# Change: Add Basic Window Recording

## Why

The application currently has no recording functionality—it's a bare Tauri scaffold. Users need the core capability to enumerate visible windows, select one, and record it to an MP4 file. This forms the foundation for all future capture modes and features.

## What Changes

- Add Windows-native screen capture using DXGI Desktop Duplication API
- Implement window enumeration to list capturable windows
- Create recording state machine (idle → recording → saving)
- Encode captured frames to H.264/MP4 using FFmpeg
- Build minimal UI with window list and record/stop button

## Impact

- Affected specs: `window-capture` (new), `recording-control` (new)
- Affected code:
  - `src-tauri/src/` - New Rust modules for capture and encoding
  - `src/` - New TypeScript UI for window selection and recording controls
  - `src-tauri/Cargo.toml` - New dependencies for Windows APIs and FFmpeg
  - `index.html` - Updated UI markup
