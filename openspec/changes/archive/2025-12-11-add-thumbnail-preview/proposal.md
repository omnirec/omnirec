# Change: Add Pre-Capture Thumbnail Support

## Why

Users currently select windows and displays from text-only lists, making it difficult to identify the correct target—especially when multiple windows have similar titles. Thumbnails provide immediate visual context, reducing selection errors and improving the user experience.

## What Changes

- Add thumbnail capture capability to the platform abstraction layer
- Display thumbnails in window and display selection lists
- Show region preview in the content area when region selector is activated
- Implement thumbnail refresh every 5 seconds while lists are visible
- **Linux/Wayland**: Full implementation using existing PipeWire/portal flow
- **Windows/macOS**: Stub implementations (to be completed separately)

## Impact

- Affected specs: New `thumbnail-capture` capability, modifications to `platform-abstraction`
- Affected code:
  - `src-tauri/src/capture/` - New thumbnail module and platform implementations
  - `src-tauri/src/capture/linux/` - Full Linux thumbnail implementation
  - `src-tauri/src/capture/windows/`, `src-tauri/src/capture/macos/` - Stub implementations
  - `src/main.ts` - Thumbnail display and refresh logic
  - `src/styles.css` - Thumbnail styling
  - `index.html` - Updated list item structure
