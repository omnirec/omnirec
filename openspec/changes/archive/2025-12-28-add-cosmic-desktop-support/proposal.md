# Change: Add COSMIC Desktop Environment Support

## Why

Pop!_OS has introduced COSMIC, a new Wayland-based desktop environment with its own compositor (cosmic-comp). OmniRec currently only recognizes GNOME and KDE for tray-mode recording, and Hyprland for full-featured capture. COSMIC users cannot use OmniRec because it falls through to "unknown" desktop environment handling, which attempts Hyprland IPC and fails.

COSMIC uses standard xdg-desktop-portal for screen capture (like GNOME/KDE), making it compatible with the existing tray-mode workflow.

## Research: Can We Bypass the COSMIC Picker?

**Short answer: No, not without significant COSMIC-specific work.**

COSMIC has its own portal implementation (`xdg-desktop-portal-cosmic`) that handles screencast requests internally. Unlike Hyprland's XDPH which calls an external picker binary (allowing OmniRec to substitute its own), COSMIC's portal shows a built-in dialog using libcosmic (their Iced-based UI toolkit).

Key findings from analyzing `xdg-desktop-portal-cosmic`:
- The picker is tightly integrated into the portal process
- No external picker binary mechanism exists
- The dialog is implemented in `screencast_dialog.rs` using COSMIC's native widget system
- There's `restore_data` support for persisting user selections, but this requires knowing COSMIC's internal output/toplevel identifiers

**Future possibilities:**
- COSMIC could add external picker support (like Hyprland)
- We could implement COSMIC-specific IPC for window enumeration via cosmic-comp
- We could use `restore_data` if we can obtain COSMIC identifiers

For now, tray-mode is the appropriate approach.

## Known Limitation: Tray Icon Not Visible on COSMIC

COSMIC's StatusNotifierItem (SNI) host implementation has a known issue where icons from some apps are not displayed (see [cosmic-applets#1178](https://github.com/pop-os/cosmic-applets/issues/1178)). This affects many applications including Vesktop and Mullvad VPN.

**Root cause:** Tauri's `tray-icon` crate has a bug in its libappindicator usage. In `platform_impl/gtk/mod.rs`, it calls:
```rust
indicator.set_icon_full(&icon_path.to_string_lossy(), "icon");
```
This passes the **full path** (e.g., `/run/user/1000/tray-icon/tray-icon-1-0.png`) but libappindicator's `set_icon_full()` expects an **icon name** (e.g., `tray-icon-1-0`). The icon name is looked up in the theme path set via `set_icon_theme_path()`.

GNOME and KDE's SNI implementations are more tolerant and handle this incorrectly-formatted input, but COSMIC's implementation strictly follows the spec and fails to display the icon.

**Impact:** The tray icon area is clickable and functional (menu works), but the icon itself is invisible.

**Upstream fix needed:** This bug should be reported to https://github.com/tauri-apps/tray-icon/issues

**Status:** This is a bug in Tauri's tray-icon crate that manifests on COSMIC due to its stricter SNI implementation. A proper fix requires an upstream patch to tray-icon.

## What Changes

- Add COSMIC detection via `XDG_CURRENT_DESKTOP` environment variable containing "COSMIC"
- Include COSMIC in the tray-mode desktop list alongside GNOME and KDE
- Add `is_cosmic()` command for frontend desktop detection
- Update `get_desktop_environment()` to return "cosmic" for COSMIC desktops

## Impact

- Affected specs: `gnome-tray-mode`
- Affected code:
  - `src-tauri/src/lib.rs` - Desktop detection functions and tray mode setup
  - `src/main.ts` - Frontend desktop environment handling
