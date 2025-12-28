## 1. Backend Desktop Detection

- [x] 1.1 Add `is_cosmic()` Tauri command to detect COSMIC via `XDG_CURRENT_DESKTOP`
- [x] 1.2 Update `get_desktop_environment()` to return "cosmic" for COSMIC desktops
- [x] 1.3 Update `is_tray_mode_desktop()` to include COSMIC detection

## 2. Tray Mode Integration

- [x] 2.1 Ensure COSMIC triggers tray-mode setup in `setup_tray_mode()`
- [x] 2.2 Verify tray icon compatibility with COSMIC's system tray

## 3. Frontend Integration

- [x] 3.1 Update frontend to recognize "cosmic" desktop environment
- [x] 3.2 Ensure COSMIC follows same UI flow as GNOME/KDE (hidden capture tabs, tray-based recording)

## 4. Testing & Documentation

- [x] 4.1 Test on COSMIC desktop environment (or verify detection logic)
- [x] 4.2 Update README with COSMIC support mention
