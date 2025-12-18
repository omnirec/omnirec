# Change: Implement macOS Thumbnail Capture

## Why

The macOS platform currently returns `CaptureError::NotImplemented` for all thumbnail capture operations, resulting in placeholder images displayed for windows, displays, and region previews. Implementing thumbnail capture on macOS will provide visual selection feedback, improving the user experience to match what's already available on Linux and Windows.

## What Changes

- Replace stub implementation in `src-tauri/src/capture/macos/thumbnail.rs` with functional capture code
- Use `CGWindowListCreateImage` from the `core-graphics` crate (already a dependency) for immediate screenshot capture
- Implement display thumbnail capture by capturing the display and scaling the result
- Implement window thumbnail capture by capturing a specific window using its CGWindowID
- Implement region preview by capturing the display and cropping to the specified region
- Handle Retina display scaling (convert logical coordinates to physical pixels for cropping)
- Reuse existing shared thumbnail encoding utilities (`bgra_to_jpeg_thumbnail`)

## Impact

- Affected specs: `thumbnail-capture` (remove macOS stub requirement, add macOS implementation requirement)
- Affected code:
  - `src-tauri/src/capture/macos/thumbnail.rs` (~100-150 lines implementation)
  - No new dependencies required (uses existing `core-graphics`, `core-foundation`, and `image` crates)
- No breaking changes to public APIs
- No changes to Linux or Windows implementations
