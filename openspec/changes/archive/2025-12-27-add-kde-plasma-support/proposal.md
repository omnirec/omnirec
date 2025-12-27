# Change: Add KDE Plasma Desktop Support

## Why

KDE Plasma is one of the most popular Linux desktop environments, but OmniRec currently only supports Hyprland (full features) and GNOME (tray mode). Users on KDE Plasma see an "unknown" desktop environment and have limited functionality. KDE's portal (`xdg-desktop-portal-kde`) does not support custom picker binaries like Hyprland's XDPH, so we cannot bypass KDE's native source selection dialog. The best experience we can provide matches GNOME's tray mode.

## What Changes

- Detect KDE Plasma desktop environment via `XDG_CURRENT_DESKTOP`
- Activate tray mode on KDE Plasma (same as GNOME)
- Rename internal concepts from "GNOME mode" to "tray mode" for clarity
- Update `get_desktop_environment` to return "kde" for KDE Plasma
- Add `is_kde` Tauri command for frontend detection
- KDE users get: system tray icon, portal-based recording with native picker, configuration UI

## Impact

- Affected specs: `gnome-tray-mode` (modifications to generalize for KDE)
- Affected code:
  - `src-tauri/src/lib.rs` - desktop detection, tray setup
  - `src/main.ts` - frontend desktop environment handling
- No breaking changes - existing Hyprland and GNOME behavior unchanged
