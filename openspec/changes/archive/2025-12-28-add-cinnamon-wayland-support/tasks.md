## 1. Backend Detection

- [x] 1.1 Add `is_cinnamon()` command to `src-tauri/src/lib.rs`
- [x] ~~1.2 Update `is_tray_mode_desktop()` to include "X-CINNAMON" detection~~ **REVERTED** - see notes
- [x] 1.3 Update `get_desktop_environment()` to return "cinnamon" when appropriate

## 2. Frontend Detection

- [x] ~~2.1 Update `src/main.ts` to detect Cinnamon and enable tray mode UI~~ **REVERTED** - see notes

## 3. Testing

- [x] 3.1 Test on Linux Mint Cinnamon Wayland session
- [ ] ~~3.2 Verify tray icon appears on startup~~ N/A
- [ ] ~~3.3 Verify portal picker appears when recording is started~~ N/A
- [ ] ~~3.4 Verify recording completes successfully~~ N/A

## Notes

**Cinnamon Wayland support is NOT possible at this time.**

Testing revealed that Linux Mint's `xdg-desktop-portal-xapp` does NOT implement the `org.freedesktop.portal.ScreenCast` interface required for screen recording.

From `/data/xapp.portal`:
```
Interfaces=org.freedesktop.impl.portal.Wallpaper;org.freedesktop.impl.portal.Inhibit;org.freedesktop.impl.portal.Screenshot;org.freedesktop.impl.portal.Lockdown;org.freedesktop.impl.portal.Settings;org.freedesktop.impl.portal.Background;
```

**Missing:** `org.freedesktop.impl.portal.ScreenCast`

Until Linux Mint adds ScreenCast support to xdg-desktop-portal-xapp, OmniRec cannot record on Cinnamon Wayland.

**Changes Reverted:**
- Removed Cinnamon from `is_tray_mode_desktop()` 
- Removed Cinnamon from tray mode detection in frontend
- Kept `is_cinnamon()` and `get_desktop_environment()` returning "cinnamon" for future use
