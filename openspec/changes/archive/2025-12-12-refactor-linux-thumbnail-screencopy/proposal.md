# Change: Refactor Linux thumbnail capture to use wlr-screencopy protocol

## Why

The current Linux thumbnail implementation uses the Portal + PipeWire flow for single-frame captures, which introduces 200-600ms latency per thumbnail due to portal session creation, picker subprocess invocation, and PipeWire stream initialization. This is excessive for lightweight screenshot operations, causing sluggish thumbnail refresh and poor UX when browsing window/display lists.

The `wlr-screencopy-unstable-v1` Wayland protocol is purpose-built for efficient single-frame capture with ~10-50ms latency, eliminating D-Bus, portal, and PipeWire overhead entirely.

## What Changes

- **New screencopy module**: Add `src-tauri/src/capture/linux/screencopy.rs` implementing wlr-screencopy protocol
- **MODIFIED Linux Thumbnail Implementation**: Replace portal-based thumbnail capture with wlr-screencopy
- **Window thumbnails via output cropping**: Capture the output containing the window, then crop to window bounds using Hyprland IPC geometry
- **Remove portal thumbnail code**: The existing portal-based thumbnail capture will be removed (portal flow remains for recording only)
- **Leverage existing dependencies**: `wayland-protocols-wlr` and `wayland-client` already present

## Impact

- Affected specs: `thumbnail-capture` (MODIFIED: Linux implementation)
- Affected code:
  - `src-tauri/src/capture/linux/thumbnail.rs` (rewrite to use screencopy)
  - New module: `src-tauri/src/capture/linux/screencopy.rs`
  - `src-tauri/src/capture/linux/mod.rs` (expose screencopy module)
- Performance: ~10x faster thumbnail capture (from 200-600ms to 10-50ms)
- Compatibility: wlroots compositors only (Hyprland); other compositors will show placeholder thumbnails until alternative solution is implemented
