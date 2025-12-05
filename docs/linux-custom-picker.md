# Linux Custom Picker for Wayland

This document describes how the custom picker works on Linux/Wayland (specifically Hyprland) to enable seamless screen recording without the standard system picker dialog.

## Problem

On Wayland, applications cannot directly capture screen content due to security restrictions. Instead, they must go through `xdg-desktop-portal`, which:

1. Receives capture requests from applications
2. Routes them to a compositor-specific backend (e.g., xdg-desktop-portal-hyprland)
3. Shows a picker dialog for the user to select what to share
4. Returns a PipeWire stream to the application

This creates a poor UX for screen recording apps because users must select their capture target twice: once in the app's UI, and again in the system picker dialog.

## Solution

We implement a custom picker that xdg-desktop-portal-hyprland (XDPH) invokes instead of its default `hyprland-share-picker`. Our picker:

1. Queries the main screen-recorder app via IPC for the user's pre-selected capture target
2. Outputs the selection to stdout in the format XDPH expects
3. Exits immediately (no UI shown)

XDPH then uses this selection to set up the PipeWire capture stream, completely bypassing the picker dialog.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Main App (Tauri)                        │
│                                                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │  User selects   │  │  IPC Server     │  │  Portal Client  │ │
│  │  capture target │──▶│  (stores        │◀─│  (ashpd)        │ │
│  │  in UI          │  │   selection)    │  │                 │ │
│  └─────────────────┘  └────────┬────────┘  └────────┬────────┘ │
│                                │                     │          │
└────────────────────────────────┼─────────────────────┼──────────┘
                                 │                     │
                            Unix Socket           D-Bus call
                                 │                     │
                                 ▼                     ▼
┌────────────────────────┐    Query    ┌─────────────────────────┐
│  screen-recorder-picker│◀────────────│  xdg-desktop-portal     │
│  (our custom picker)   │             └───────────┬─────────────┘
│                        │                         │
│  1. Query IPC server   │                  Routes to XDPH
│  2. Output to stdout:  │                         │
│     [SELECTION]/...    │                         ▼
│  3. Exit               │             ┌─────────────────────────┐
└────────────────────────┘             │  xdg-desktop-portal-    │
         │                             │  hyprland (XDPH)        │
         │                             │                         │
         └─────── stdout ─────────────▶│  - Parses our output    │
                                       │  - Sets up PipeWire     │
                                       │  - Returns node ID      │
                                       └─────────────────────────┘
```

## Flow

1. **User selects capture target in app UI** (e.g., clicks on "DP-1" monitor)
2. **Main app stores selection** in IPC server state
3. **User clicks Record**
4. **Main app calls portal** via ashpd crate (CreateSession → SelectSources → Start)
5. **xdg-desktop-portal routes to XDPH**
6. **XDPH invokes our picker** (configured via `~/.config/hypr/xdph.conf`)
7. **Picker connects to main app** via Unix socket at `$XDG_RUNTIME_DIR/screen-recorder/picker.sock`
8. **Picker queries for selection** using JSON protocol
9. **Main app returns selection** (source_type, source_id, geometry)
10. **Picker outputs to stdout**: `[SELECTION]/screen:DP-1`
11. **XDPH parses output** and sets up PipeWire capture
12. **Portal returns PipeWire node ID** to main app
13. **Recording begins** - no picker dialog was shown!

## IPC Protocol

The picker communicates with the main app over a Unix socket using JSON messages.

**Socket path:** `$XDG_RUNTIME_DIR/screen-recorder/picker.sock`

### Request (Picker → Main App)

```json
{"type": "query_selection"}
```

### Response (Main App → Picker)

**Monitor selection:**
```json
{
  "type": "selection",
  "source_type": "monitor",
  "source_id": "DP-1"
}
```

**Window selection:**
```json
{
  "type": "selection",
  "source_type": "window",
  "source_id": "0x55a1b2c3d4e5"
}
```

**Region selection:**
```json
{
  "type": "selection",
  "source_type": "region",
  "source_id": "DP-1",
  "geometry": {"x": 100, "y": 200, "width": 800, "height": 600}
}
```

**No selection:**
```json
{"type": "no_selection"}
```

## Picker Output Format

The picker outputs to stdout in the format XDPH expects (same as `hyprland-share-picker`):

| Type | Format | Example |
|------|--------|---------|
| Screen | `[SELECTION]/screen:<output>` | `[SELECTION]/screen:DP-1` |
| Window | `[SELECTION]/window:<handle>` | `[SELECTION]/window:12345678` |
| Region | `[SELECTION]/region:<output>@<x>,<y>,<w>,<h>` | `[SELECTION]/region:DP-1@100,200,800,600` |

## Installation

### 1. Build the picker

```bash
cd src-picker
cargo build --release
sudo cp target/release/screen-recorder-picker /usr/local/bin/
```

### 2. Configure XDPH

Copy the config file:
```bash
cp resources/linux/xdph.conf ~/.config/hypr/xdph.conf
```

Or manually add to your existing `~/.config/hypr/xdph.conf`:
```
screencopy {
    custom_picker_binary = screen-recorder-picker
}
```

### 3. Restart XDPH

```bash
systemctl --user restart xdg-desktop-portal-hyprland
```

## Error Handling

- **Main app not running:** Picker exits with error code, XDPH cancels the portal request
- **No selection available:** Picker exits with error, XDPH cancels the request
- **IPC connection failure:** Picker outputs error to stderr and exits with error code

## Requirements

- xdg-desktop-portal-hyprland (XDPH) must be installed and running
- Hyprland compositor (for window/monitor enumeration via Hyprland IPC)
- Main screen-recorder app must be running before recording is initiated

## Files

| File | Purpose |
|------|---------|
| `src-picker/src/main.rs` | Picker entry point |
| `src-picker/src/ipc_client.rs` | IPC client for querying main app |
| `src-tauri/src/capture/linux/ipc_server.rs` | IPC server in main app |
| `resources/linux/xdph.conf` | XDPH configuration file |
