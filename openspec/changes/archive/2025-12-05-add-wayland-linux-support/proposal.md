# Change: Add Wayland/Linux Support (Hyprland)

## Why

The screen recorder currently only supports Windows. To fulfill the project's cross-platform goals and serve Linux users, we need native Wayland support. Wayland's security model requires user consent for screen capture via xdg-desktop-portal, but standard portal dialogs are generic and disconnected from the app experience.

Our approach: implement a **custom headless picker service** that replaces the default portal picker (e.g., `hyprland-share-picker`). This picker runs as a background service and communicates with our main app via IPC. The user selects windows/displays/regions in the main app UI using Hyprland-native APIs - when they click Record, the picker auto-approves the portal request. No separate portal dialog is ever shown.

Initial implementation targets Hyprland compositor specifically, leveraging its IPC APIs for window/display enumeration.

## What Changes

- **NEW: Headless picker service** (`screen-recorder-picker`) - a separate executable that:
  - Registers as the xdg-desktop-portal picker for ScreenCast
  - Starts with the desktop environment (systemd user service)
  - Communicates with main app via Unix socket IPC
  - Auto-approves portal requests when main app provides selection
  - Falls back to denying requests if main app is not running (for now)

- **NEW: Main app Wayland backend** using:
  - Hyprland IPC for window/display enumeration (native, no portal)
  - Portal client (ashpd) to initiate capture requests
  - Unix socket server for picker IPC
  - PipeWire for video/audio capture

- **NEW: Hyprland integration** for window/display enumeration via Hyprland IPC

- **COMPLETED: Platform abstraction layer** *(implemented in `refactor-platform-abstraction` change, archived 2025-12-04)*

- Frontend remains largely unchanged - same UI for both platforms

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              User                                        │
│                                │                                         │
│                    Selects window/display/region                         │
│                                │                                         │
│                                ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                     Main App (Tauri)                             │    │
│  │                                                                  │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐ │    │
│  │  │ Hyprland IPC │  │ IPC Server   │  │ Portal Client (ashpd)  │ │    │
│  │  │ (enumerate)  │  │ (Unix sock)  │  │ (request capture)      │ │    │
│  │  └──────┬───────┘  └──────┬───────┘  └───────────┬────────────┘ │    │
│  │         │                 │                      │               │    │
│  └─────────┼─────────────────┼──────────────────────┼───────────────┘    │
│            │                 │                      │                    │
│   Window/Display             │                      │                    │
│   enumeration          IPC channel           Portal D-Bus                │
│   (direct)                   │                      │                    │
│            │                 │                      ▼                    │
│            │                 │         ┌────────────────────────┐        │
│            │                 │         │   xdg-desktop-portal   │        │
│            │                 │         │   (system service)     │        │
│            │                 │         └───────────┬────────────┘        │
│            │                 │                     │                     │
│            │                 │              Routes to picker             │
│            │                 │                     │                     │
│            │                 │                     ▼                     │
│            │                 │    ┌────────────────────────────────┐     │
│            │                 └───►│  screen-recorder-picker        │     │
│            │                      │  (headless systemd service)    │     │
│            │                      │                                │     │
│            │                      │  - Receives portal request     │     │
│            │                      │  - Queries main app via IPC    │     │
│            │                      │  - Returns approved source     │     │
│            │                      └───────────────┬────────────────┘     │
│            │                                      │                      │
│            │                               Auto-approve                  │
│            │                                      │                      │
│            │                                      ▼                      │
│            │                           ┌───────────────────┐             │
│            │                           │  PipeWire stream  │             │
│            │                           │  established      │             │
│            │                           └─────────┬─────────┘             │
│            │                                     │                       │
│            ▼                                     ▼                       │
│  ┌───────────────────────────────────────────────────────────────┐      │
│  │                    Recording begins                            │      │
│  └───────────────────────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────────────────────┘
```

### Portal Flow (Invisible to User)

```
User selects display in app UI (via Hyprland IPC)
         ↓
App stores selection and starts IPC server
         ↓
User clicks Record
         ↓
App calls portal CreateSession/SelectSources/Start
         ↓
xdg-desktop-portal routes to screen-recorder-picker
         ↓
Picker queries main app via Unix socket IPC
         ↓
Main app responds with selected source details
         ↓
Picker auto-approves with correct PipeWire node
         ↓
Portal returns PipeWire stream to app
         ↓
Recording begins
```

### Phased Implementation

1. **Phase 1**: Picker service + IPC + display capture
2. **Phase 2**: Window capture using Hyprland IPC
3. **Phase 3**: Region capture with overlay adaptation
4. **Phase 4**: Audio capture via PipeWire

## Impact

- Affected specs: 
  - `window-capture` (platform-specific implementations)
  - `display-capture` (platform-specific implementations)
  - `region-capture` (platform-specific implementations)
  - `recording-control` (transparent portal flow)
  - NEW `wayland-capture` (Wayland-specific capture)
  - NEW `wayland-portal` (picker service + IPC protocol)
  - `platform-abstraction` (already implemented - Linux stub to be extended)

- Affected code:
  - `src-tauri/src/capture/linux/` - Extend with Hyprland IPC, portal client, IPC server
  - NEW `src-picker/` - Separate Rust binary for headless picker service
  - `src-tauri/Cargo.toml` - Linux dependencies
  - NEW systemd user service file for picker

- New dependencies:
  - `pipewire` / `libspa` crates for PipeWire integration
  - `ashpd` crate for portal client (main app)
  - `zbus` crate for portal backend (picker service)
  - `hyprland` crate for Hyprland IPC
  - `tokio` with Unix socket support for IPC

- New files to install:
  - `screen-recorder-picker` binary → `/usr/bin/` or `~/.local/bin/`
  - `screen-recorder.portal` → `/usr/share/xdg-desktop-portal/portals/`
  - `screen-recorder-picker.service` → `~/.config/systemd/user/`
  - Portal config → `~/.config/xdg-desktop-portal/hyprland-portals.conf`
