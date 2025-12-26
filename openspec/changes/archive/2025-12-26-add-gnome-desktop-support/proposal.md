# Change: Add GNOME Desktop Support

## Why

GNOME's xdg-desktop-portal implementation cannot be effectively overridden like Hyprland's custom picker. The current UI-first approach (where users select a capture target in the app before recording) doesn't work on GNOME because the portal always shows its own picker dialog. This change introduces a tray-based workflow for GNOME that embraces the native portal picker while providing quick access to recording controls.

## What Changes

- **New capability: gnome-tray-mode** - GNOME-specific tray icon workflow
  - Add system tray icon when running on GNOME desktop
  - Tray menu with: Start Recording, Stop Recording, Configuration, About, Exit
  - Start Recording invokes the standard GNOME screen recording portal picker
  - Stop Recording stops any active recording (disabled when not recording)
  - Tray icon changes to red dot when recording is active
  - Configuration/About menu items open the main window to the respective tab
  - Exit quits the application entirely and removes the tray icon
  - On GNOME, hide Window/Region/Display tabs (portal handles selection)
  - On GNOME, closing the main window hides it but keeps backend running

- **New capability: about-tab** - Cross-platform About tab
  - Add About tab to the main window (all platforms)
  - Display: version, website link, GitHub link, copyright notice, license info

## Impact

- Affected specs: 
  - New: `gnome-tray-mode` (GNOME-specific behavior)
  - New: `about-tab` (cross-platform)
  - Modified: `ui-theme` (new About tab in tab bar)
  - Modified: `app-configuration` (GNOME-specific tab visibility)
- Affected code:
  - `src-tauri/src/lib.rs` - GNOME detection, tray icon setup, window lifecycle
  - `src/main.ts` - Tab visibility logic, About tab rendering
  - `index.html` - About tab markup
  - `src/styles.css` - About tab styles
  - `Cargo.toml` - tray-icon plugin dependency
