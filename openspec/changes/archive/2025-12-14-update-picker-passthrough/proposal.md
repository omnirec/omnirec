# Change: Add pass-through capability to custom picker

## Why

Currently, the custom `omnirec-picker` is configured as the system-wide picker for xdg-desktop-portal-hyprland. When any application (OBS, Zoom, Discord, etc.) requests screen recording permission, the picker queries OmniRec via IPC. If OmniRec isn't running or has no active selection, the picker exits with failure, **blocking all other applications from accessing screen capture**.

Users need the ability to use other screen recording applications alongside OmniRec without manually switching the XDPH picker configuration.

## What Changes

- Modify `omnirec-picker` to detect when OmniRec is not available (IPC connection fails)
- On fallback condition, execute the standard `hyprland-share-picker` binary instead of failing
- Pass through all environment variables and arguments to the standard picker
- Capture and forward the standard picker's output to XDPH

## Impact

- Affected specs: `wayland-portal`
- Affected code: `src-picker/src/main.rs`
- User experience: Other apps can request screen capture normally when OmniRec is not running
