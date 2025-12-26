# Tasks: Add GNOME Desktop Support

## 1. About Tab (Cross-Platform)

- [x] 1.1 Add About tab button to tab bar in `index.html` (info icon, after gear icon)
- [x] 1.2 Add About view section markup in `index.html` with logo, version, links, copyright, license
- [x] 1.3 Add About tab styles in `src/styles.css`
- [x] 1.4 Add About tab TypeScript logic in `src/main.ts` (tab switching, external link handling)
- [x] 1.5 Define website and GitHub URLs as constants
- [x] 1.6 Add `shell-open` capability for opening external links (using tauri-plugin-opener)

## 2. GNOME Detection

- [x] 2.1 Add `is_gnome()` detection function in Rust (check `XDG_CURRENT_DESKTOP`)
- [x] 2.2 Add `get_desktop_environment` Tauri command to expose detection to frontend
- [x] 2.3 Add frontend detection and store GNOME mode state

## 3. System Tray Implementation

- [x] 3.1 Add `tray-icon` feature to tauri dependency in Cargo.toml
- [x] 3.2 Create tray icon assets (symbolic icons for Linux: omnirec-symbolic-*.png, omnirec-recording-*.png)
- [x] 3.3 Initialize tray icon on GNOME startup
- [x] 3.4 Implement tray menu structure (Record Screen/Window, Stop, Config, About, Exit)
- [x] 3.5 Implement tray menu event handlers
- [x] 3.6 Add tray icon state updates (normal/recording icons)

## 4. GNOME Recording Flow

- [x] 4.1 Implement "Record Screen/Window" tray action that invokes portal with native picker
- [x] 4.2 Handle portal picker completion (start recording)
- [x] 4.3 Handle portal picker cancellation (return to idle)
- [x] 4.4 Implement "Stop Recording" tray action
- [x] 4.5 Update tray icon on recording state changes

## 5. GNOME Window Behavior

- [x] 5.1 Window close hides app (tray keeps running) - handled via tray-based workflow
- [x] 5.2 Implement "show window with specific tab" functionality for tray menu
- [x] 5.3 Window visibility managed via tray menu (Config/About show window)
- [x] 5.4 Hide main window on GNOME startup (start with tray only)

## 6. GNOME Tab Visibility

- [x] 6.1 Add frontend logic to hide Window/Region/Display tabs on GNOME
- [x] 6.2 Set Config as default tab on GNOME when window opens
- [x] 6.3 Ensure tab navigation works correctly with reduced tab set

## 7. Exit Handling

- [x] 7.1 Implement Exit tray menu action (stop recording if active, quit app)
- [x] 7.2 Tray icon cleanup handled by Tauri on exit
- [x] 7.3 Resource cleanup handled via existing recording stop logic

## 8. Testing & Validation

- [x] 8.1 Test on GNOME desktop (tray icon appears, menu works)
- [x] 8.2 Test recording flow via tray (portal picker, recording, stop)
- [x] 8.3 Test window hide/show behavior
- [x] 8.4 Test Exit properly quits application
- [x] 8.5 Test About tab on all platforms
- [x] 8.6 Verify Hyprland behavior unchanged
- [x] 8.7 Run `cargo clippy` and `pnpm exec tsc --noEmit`

## Notes

### Region Recording - Not Implemented

Region recording was investigated but determined to be not feasible on GNOME/Wayland due to fundamental platform limitations:
- On Wayland, applications cannot determine their own window position (`outerPosition()` returns 0,0)
- Without window position, region coordinates cannot be mapped to screen coordinates
- The portal always shows a full share picker, not just an authorization dialog

Decision: Region recording removed from GNOME mode. Users can record full screens or windows only.

### Dependencies Added

- `libappindicator-gtk3` - Required for system tray icon support on Linux/GNOME
