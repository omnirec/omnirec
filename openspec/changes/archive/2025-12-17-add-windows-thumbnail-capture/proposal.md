# Change: Implement Windows Thumbnail Capture

## Why

The Windows platform currently returns `CaptureError::NotImplemented` for all thumbnail capture operations, resulting in placeholder images displayed for windows, displays, and region previews. Implementing thumbnail capture on Windows will provide visual selection feedback, improving the user experience to match what's already available on Linux.

## What Changes

- Replace stub implementation in `src-tauri/src/capture/windows/thumbnail.rs` with functional capture code
- Use `windows-capture` crate (already a dependency) for single-frame capture via Windows.Graphics.Capture API
- Implement display thumbnail capture by capturing the monitor and scaling the result
- Implement window thumbnail capture by capturing the window directly using its HWND
- Implement region preview by capturing the monitor and cropping to the specified region
- Reuse existing shared thumbnail encoding utilities (`bgra_to_jpeg_thumbnail`)

## Impact

- Affected specs: `thumbnail-capture` (remove Windows stub requirement, add Windows implementation requirement)
- Affected code:
  - `src-tauri/src/capture/windows/thumbnail.rs` (~100-150 lines implementation)
  - No new dependencies required (uses existing `windows-capture` and `image` crates)
- No breaking changes to public APIs
- No changes to Linux or macOS implementations
