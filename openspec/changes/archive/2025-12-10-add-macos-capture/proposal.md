# Change: Add macOS Screen Capture Support

## Why

OmniRec currently supports Windows and Linux screen capture but has only stub implementations for macOS. Users on macOS cannot record screens, windows, or regions. This change implements full macOS capture support using Apple's ScreenCaptureKit framework (macOS 12.3+), enabling feature parity across all desktop platforms.

## What Changes

- Implement `MacOSBackend` with full `CaptureBackend`, `WindowEnumerator`, `MonitorEnumerator`, and `HighlightProvider` trait implementations
- Add ScreenCaptureKit integration for high-performance frame capture
- Add Core Graphics integration for window/display enumeration
- Add macOS-specific dependencies to Cargo.toml (conditionally compiled)
- Implement permission handling for screen recording authorization

## Impact

- Affected specs: `platform-abstraction` (existing macOS scenarios now functional), new `macos-capture` capability
- Affected code:
  - `src-tauri/src/capture/macos/mod.rs` - Main backend implementation
  - `src-tauri/src/capture/macos/window_list.rs` - Window enumeration (new)
  - `src-tauri/src/capture/macos/monitor_list.rs` - Monitor enumeration (new)
  - `src-tauri/src/capture/macos/recorder.rs` - ScreenCaptureKit capture (new)
  - `src-tauri/src/capture/macos/region.rs` - Region capture (new)
  - `src-tauri/src/capture/macos/highlight.rs` - Visual highlight feedback (new)
  - `src-tauri/Cargo.toml` - macOS dependencies
- No changes to Windows or Linux code paths
- Frontend code unchanged (already abstracts over platform)
