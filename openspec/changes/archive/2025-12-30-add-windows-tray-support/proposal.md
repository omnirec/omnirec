# Change: Add Windows System Tray Support

## Why

Windows users currently have no system tray icon. The tray provides quick access to recording controls and status indication without requiring the main window to be visible. Linux already has full tray support, and the Windows module is a stub waiting to be implemented.

## What Changes

- Implement full system tray functionality on Windows
- Add tray icon with context menu (Start/Stop Recording, Configuration, About, Exit)
- Add recording state indicator (icon changes when recording)
- Remove Windows from the "stub" requirement since it will have a full implementation

## Impact

- Affected specs: `system-tray`
- Affected code: `src-tauri/src/tray/windows.rs`, `src-tauri/src/tray/mod.rs`
