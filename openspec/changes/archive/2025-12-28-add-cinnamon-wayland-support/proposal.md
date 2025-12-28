# Change: Add Linux Mint Cinnamon Wayland Support

## Status: BLOCKED

**Cinnamon Wayland support is not possible at this time.**

Linux Mint's `xdg-desktop-portal-xapp` does NOT implement the `org.freedesktop.portal.ScreenCast` interface required for screen recording via portals.

## Investigation Results

Testing on Linux Mint Cinnamon Wayland revealed:

1. **Portal Error:** `A portal frontend implementing 'org.freedesktop.portal.ScreenCast' was not found`

2. **Root Cause:** The `xdg-desktop-portal-xapp` portal backend only implements:
   - Wallpaper
   - Inhibit  
   - Screenshot
   - Lockdown
   - Settings
   - Background

   **ScreenCast is NOT implemented.**

3. **Source:** https://github.com/linuxmint/xdg-desktop-portal-xapp/blob/master/data/xapp.portal

## What Would Be Needed

For OmniRec to work on Cinnamon Wayland, Linux Mint would need to either:
1. Add ScreenCast support to xdg-desktop-portal-xapp
2. Configure Cinnamon to use xdg-desktop-portal-gnome or xdg-desktop-portal-wlr as a fallback for ScreenCast

## Changes Made

- Added `is_cinnamon()` detection command (for future use)
- Added "cinnamon" to `get_desktop_environment()` (for future use)
- **NOT added** to tray-mode desktops (would cause runtime errors)

## Original Proposal

### Why

Linux Mint 22+ introduces Cinnamon with Wayland session support. OmniRec currently supports Hyprland (custom picker) and tray-mode desktops (GNOME, KDE, COSMIC) that use the standard portal picker.

### What Was Planned

- Add Cinnamon detection to tray-mode desktop environments
- Cinnamon Wayland sessions would use the same portal-based workflow as GNOME/KDE
- Users would interact via system tray icon and native portal picker

### Why It Doesn't Work

The portal-based recording requires the ScreenCast interface, which xdg-desktop-portal-xapp does not provide.
