# Change: Add Window and Display Highlighting for Linux

## Why

When a user selects a window or display from the capture list on Linux, there is no visual feedback to show which target is selected. Windows and macOS already provide a brief highlight border that flashes around the selected window or display. This feature helps users confirm their selection before recording.

## What Changes

- Implement `show_highlight` for `LinuxBackend` using the `wlr-layer-shell` Wayland protocol
- Create a transparent overlay surface on the `overlay` layer positioned over the target area
- Apply the same visual style as other platforms (blue #2196F3 border, ~800ms duration)
- The highlight is non-interactive (no keyboard interactivity) and auto-dismisses after the timeout
- Gracefully degrades to no-op on compositors without layer-shell support

## Impact

- Affected specs: `wayland-capture`
- Affected code:
  - `src-tauri/src/capture/linux/mod.rs` - Update `HighlightProvider` implementation
  - `src-tauri/src/capture/linux/highlight.rs` - New file for highlight implementation
  - `src-tauri/Cargo.toml` - Add `wayland-client` and `wayland-protocols-wlr` dependencies
