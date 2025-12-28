# Change: Refactor Tray Abstraction

## Why

The current tray implementation is tightly coupled to Linux "tray-mode" desktops (GNOME, KDE, COSMIC) as a workaround for portals that don't support custom pickers. However, system tray is a standard feature on all major desktop platforms (Windows, macOS, Linux) and should be a first-class feature rather than a secondary mode. Users on Windows and macOS would benefit from tray-based recording controls for quick access without keeping the main window visible.

## What Changes

- **New capability: system-tray** - Cross-platform system tray abstraction
  - Define platform-agnostic tray interface with recording controls
  - Implement tray backend for Windows (stub for now)
  - Implement tray backend for macOS (stub for now)
  - Refactor Linux tray to use shared abstraction
  - Tray enabled by default on all platforms that support it
  - Tray menu structure consistent across platforms

- **Modified capability: gnome-tray-mode** - Linux portal-mode specific behavior
  - Retain Linux-specific "tray-mode" logic for portal-based desktops
  - Reference shared system-tray capability for base functionality
  - Keep portal recording and hidden capture tabs as portal-mode features

## Impact

- Affected specs:
  - New: `system-tray` (cross-platform tray abstraction)
  - Modified: `gnome-tray-mode` (refactored to reference system-tray)
- Affected code:
  - `src-tauri/src/tray.rs` - Refactor to cross-platform structure
  - `src-tauri/src/tray/mod.rs` - Platform abstraction module
  - `src-tauri/src/tray/linux.rs` - Linux-specific tray code
  - `src-tauri/src/tray/windows.rs` - Windows stub
  - `src-tauri/src/tray/macos.rs` - macOS stub
  - `src-tauri/src/lib.rs` - Tray initialization for all platforms
