# Change: Implement macOS System Menu (Tray) App Support

## Why

macOS currently has a stub tray implementation, leaving users without system tray functionality. The menu bar is a key part of the macOS user experience for background applications, and OmniRec should support it to match the experience on Linux and Windows.

## What Changes

- **macOS tray icon**: Add monochromatic (template) icon that adapts to light/dark system themes
- **Recording indicator**: Show red dot overlay when recording is active
- **Menu items with macOS-specific behavior**:
  - "Record Screen/Window" - shows and activates (brings to front) the main app window
  - "Stop Recording" - stops current recording (only enabled during recording)
  - "Transcription" - shows/activates transcription window if active, otherwise shows error
  - "Configuration" - shows main window with Configuration tab active
  - "About" - shows main window with About tab active
  - "Exit" - quits the application
- **Cross-platform addition**: Add "Transcription" menu item to Linux and Windows tray menus

## Impact

- Affected specs: `system-tray`
- Affected code:
  - `src-tauri/src/tray/macos.rs` - full implementation replacing stub
  - `src-tauri/src/tray/mod.rs` - add `TRANSCRIPTION` menu ID and label
  - `src-tauri/src/tray/linux.rs` - add transcription menu item
  - `src-tauri/src/tray/windows.rs` - add transcription menu item
  - `src-tauri/icons/tray/` - add macOS template icons
  - `src/main.ts` - add `tray-show-transcription` event listener
