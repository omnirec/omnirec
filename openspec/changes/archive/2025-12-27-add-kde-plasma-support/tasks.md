# Tasks

## 1. Backend Changes

- [x] 1.1 Add KDE Plasma detection via `XDG_CURRENT_DESKTOP` containing "KDE"
- [x] 1.2 Add `is_kde()` Tauri command
- [x] 1.3 Update `get_desktop_environment()` to return "kde" for KDE Plasma
- [x] 1.4 Update `is_tray_mode_desktop()` helper to include KDE detection
- [x] 1.5 Ensure tray icon setup triggers for KDE (same as GNOME)

## 2. Frontend Changes

- [x] 2.1 Add "kde" to `DesktopEnvironment` type
- [x] 2.2 Update `detectDesktopEnvironment()` to handle KDE
- [x] 2.3 Apply tray mode UI changes for KDE (same as GNOME: hide capture tabs)

## 3. Validation

- [x] 3.1 Run `cargo clippy` and fix any warnings
- [x] 3.2 Run `pnpm exec tsc --noEmit` and fix any type errors
- [x] 3.3 Verify GNOME behavior unchanged
- [x] 3.4 Verify Hyprland behavior unchanged
