# Design: Wayland/Linux Support (Hyprland)

## Context

Wayland compositors enforce strict security: applications cannot capture screen content without explicit user consent via xdg-desktop-portal. The portal presents a system dialog for the user to select what to share. This differs fundamentally from Windows where apps can directly access window/display content.

**Stakeholders:**
- Linux/Hyprland users wanting screen recording
- Developers maintaining cross-platform codebase

**Constraints:**
- Must satisfy Wayland security model (portal consent required)
- PipeWire is the standard capture mechanism on modern Linux
- Hyprland provides IPC for window/output enumeration
- Cannot support arbitrary Wayland compositors initially
- Portal picker is a separate process from the main app

**Prerequisite Completed:**
- Platform abstraction layer was implemented in the `refactor-platform-abstraction` change (archived 2025-12-04)
- The `LinuxBackend` stub exists at `src-tauri/src/capture/linux/mod.rs` and implements all required traits
- This change will replace the stub implementation with actual Wayland/Hyprland capture

## Goals / Non-Goals

**Goals:**
- Full-featured screen recording on Hyprland (window, region, display, audio)
- Seamless UX where portal picker is invisible to the user
- Clean platform abstraction that doesn't pollute shared code
- Maintain Windows functionality unchanged

**Non-Goals:**
- Support for other Wayland compositors (GNOME, KDE, sway) in this change
- X11 support
- Fallback mechanisms for non-Hyprland environments

## Architecture Overview

The solution requires two separate processes that communicate via IPC:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                         Main App (Tauri)                                │ │
│  │                                                                         │ │
│  │  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────────────┐   │ │
│  │  │   Frontend UI   │  │  Hyprland IPC   │  │   Portal Client      │   │ │
│  │  │ (window/display │  │  (enumerate     │  │   (ashpd - request   │   │ │
│  │  │  selection)     │  │   windows &     │  │    screencast)       │   │ │
│  │  │                 │  │   monitors)     │  │                      │   │ │
│  │  └────────┬────────┘  └────────┬────────┘  └──────────┬───────────┘   │ │
│  │           │                    │                      │                │ │
│  │           │                    ▼                      │                │ │
│  │           │           ┌─────────────────┐             │                │ │
│  │           │           │  IPC Server     │             │                │ │
│  │           └──────────►│  (Unix socket)  │             │                │ │
│  │                       │  Holds current  │             │                │ │
│  │                       │  selection      │             │                │ │
│  │                       └────────┬────────┘             │                │ │
│  │                                │                      │                │ │
│  └────────────────────────────────┼──────────────────────┼────────────────┘ │
│                                   │                      │                  │
│                              Unix Socket            D-Bus call              │
│                              (query selection)      (portal request)        │
│                                   │                      │                  │
│                                   │                      ▼                  │
│                                   │         ┌────────────────────────┐      │
│                                   │         │   xdg-desktop-portal   │      │
│                                   │         │   (system service)     │      │
│                                   │         └───────────┬────────────┘      │
│                                   │                     │                   │
│                                   │              Routes to our              │
│                                   │              registered picker          │
│                                   │                     │                   │
│  ┌────────────────────────────────┼─────────────────────┼────────────────┐  │
│  │                                │                     ▼                │  │
│  │              ┌─────────────────┴─────────────────────────────┐        │  │
│  │              │        screen-recorder-picker                  │        │  │
│  │              │        (headless systemd service)              │        │  │
│  │              │                                                │        │  │
│  │              │  1. Receives ScreenCast request via D-Bus      │        │  │
│  │              │  2. Connects to main app IPC socket            │        │  │
│  │              │  3. Queries current selection                  │        │  │
│  │              │  4. Auto-approves with selected source         │        │  │
│  │              │  5. Returns PipeWire node to portal            │        │  │
│  │              │                                                │        │  │
│  │              │  If main app not running: deny request         │        │  │
│  │              └─────────────────┬─────────────────────────────┘        │  │
│  │                                │                                      │  │
│  │                  Picker Service (separate process)                    │  │
│  └────────────────────────────────┼──────────────────────────────────────┘  │
│                                   │                                         │
│                            Auto-approved                                    │
│                            PipeWire stream                                  │
│                                   │                                         │
│                                   ▼                                         │
│                        ┌───────────────────┐                                │
│                        │  Recording begins │                                │
│                        └───────────────────┘                                │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Decisions

### Decision 1: Separate Picker Service (Headless)

**What:** Implement the portal picker as a separate executable (`screen-recorder-picker`) that runs as a systemd user service, completely headless with no UI.

**Why:** 
- xdg-desktop-portal spawns pickers as separate processes via D-Bus activation
- Cannot embed picker logic directly in main app (different process lifecycle)
- Picker must be available before main app starts (registered with portal)
- Headless design allows the main app to own all UI/UX

**How:**
- `screen-recorder-picker` binary implements `org.freedesktop.impl.portal.ScreenCast`
- Registered via `.portal` file in `/usr/share/xdg-desktop-portal/portals/`
- Runs as systemd user service, started on login
- Communicates with main app via Unix domain socket

**Trade-offs:**
- Two binaries to build, install, and maintain
- IPC adds complexity
- Must handle case where main app isn't running

### Decision 2: Unix Socket IPC Between App and Picker

**What:** Use Unix domain socket for communication between main app and picker service.

**Why:**
- Simple, reliable IPC mechanism
- No need for D-Bus complexity for app-internal communication
- Works well with async Rust (tokio)
- Socket can be in `$XDG_RUNTIME_DIR` for proper permissions

**Protocol:**
```
Socket path: $XDG_RUNTIME_DIR/screen-recorder/picker.sock

Messages (JSON-based):

1. Picker → App: QuerySelection
   Request: { "type": "query_selection" }
   Response: { 
     "type": "selection",
     "source_type": "monitor" | "window" | "region",
     "source_id": "<hyprland monitor name or window address>",
     "geometry": { "x": 0, "y": 0, "width": 1920, "height": 1080 }  // for region
   }
   
   If no selection: { "type": "no_selection" }

2. App → Picker: SelectionUpdated (optional, for push model)
   { "type": "selection_updated", ... }
```

**Trade-offs:**
- Requires main app to run IPC server
- Socket cleanup on crash
- Simple protocol may need extension later

### Decision 3: Hyprland IPC for Enumeration (Direct, No Portal)

**What:** Use Hyprland's IPC socket to enumerate windows and outputs directly, bypassing the portal for enumeration.

**Why:**
- Portal enumeration would trigger picker, creating circular dependency
- Hyprland IPC provides rich, real-time window/output info
- Direct access means faster, more responsive UI
- Can get window geometry for region-from-window feature

**How:**
- Connect to Hyprland IPC socket (`$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock`)
- Query `monitors` for display list (JSON output)
- Query `clients` for window list (JSON output)
- Map to `MonitorInfo` and `WindowInfo` types

**Commands:**
```bash
# Monitor enumeration
hyprctl monitors -j

# Window enumeration  
hyprctl clients -j
```

**Trade-offs:**
- Tightly couples to Hyprland (acceptable per scope)
- Won't work on other compositors
- Need to handle Hyprland not running

### Decision 4: Portal Client in Main App (ashpd)

**What:** Use `ashpd` crate to make portal requests from the main app.

**Why:**
- Well-tested, maintained Rust crate
- Handles D-Bus complexity
- Provides async API compatible with tokio

**Flow:**
1. App calls `ashpd::desktop::screencast::Screencast::new()`
2. Creates session, selects sources, starts
3. Portal routes to our picker (which auto-approves)
4. App receives PipeWire node ID
5. App connects to PipeWire for frames

### Decision 5: Platform Abstraction via Traits (IMPLEMENTED)

**Status:** Completed in `refactor-platform-abstraction` change (archived 2025-12-04).

The following traits are defined in `src-tauri/src/capture/mod.rs`:

```rust
pub trait CaptureBackend: Send + Sync {
    fn start_window_capture(&self, window_handle: isize) -> Result<(FrameReceiver, StopHandle), CaptureError>;
    fn start_region_capture(&self, region: CaptureRegion) -> Result<(FrameReceiver, StopHandle), CaptureError>;
    fn start_display_capture(&self, monitor_id: String, width: u32, height: u32) -> Result<(FrameReceiver, StopHandle), CaptureError>;
}

pub trait WindowEnumerator: Send + Sync {
    fn list_windows(&self) -> Result<Vec<WindowInfo>, EnumerationError>;
}

pub trait MonitorEnumerator: Send + Sync {
    fn list_monitors(&self) -> Result<Vec<MonitorInfo>, EnumerationError>;
}
```

The `LinuxBackend` will implement these using Hyprland IPC for enumeration and portal + PipeWire for capture.

### Decision 6: PipeWire for Capture

**What:** Use PipeWire as the capture backend, receiving frames via the PipeWire graph.

**Why:**
- PipeWire is the standard Linux multimedia framework
- Portal screencast produces PipeWire streams
- Supports both video and audio in unified API
- Hardware acceleration support (DMA-BUF)

**How:**
- Portal handshake produces a PipeWire node ID
- Connect to PipeWire and create a stream consumer
- Receive frames as DMA-BUFs or SHM buffers
- Convert to BGRA for encoder compatibility

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Two-process complexity | Clear IPC protocol; good error handling |
| Picker not running | Systemd ensures service starts on login |
| Main app not running | Picker denies request gracefully |
| IPC socket issues | Proper cleanup; reconnection logic |
| Hyprland API changes | Pin to stable versions; document requirements |
| PipeWire performance | Use DMA-BUF zero-copy path where available |

## Module Structure

```
screen-recorder/
├── src-tauri/                      # Main Tauri app
│   └── src/
│       └── capture/
│           └── linux/
│               ├── mod.rs          # LinuxBackend implementation
│               ├── hyprland.rs     # Hyprland IPC client (NEW)
│               ├── ipc_server.rs   # Unix socket server for picker (NEW)
│               ├── portal_client.rs # Portal client using ashpd (EXISTS)
│               └── pipewire.rs     # PipeWire stream handling (NEW)
│
├── src-picker/                     # Separate picker binary (NEW)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                 # Entry point, D-Bus service setup
│       ├── portal_backend.rs       # ScreenCast D-Bus interface impl
│       └── ipc_client.rs           # Unix socket client to main app
│
└── resources/linux/                # Installation files
    ├── screen-recorder.portal      # Portal registration
    ├── screen-recorder-picker.service  # systemd user service
    └── hyprland-portals.conf       # Portal routing config
```

## IPC Protocol Detail

```
┌─────────────────┐                              ┌─────────────────┐
│    Main App     │                              │  Picker Service │
└────────┬────────┘                              └────────┬────────┘
         │                                                │
         │  App starts, creates IPC server                │
         │  at $XDG_RUNTIME_DIR/screen-recorder/picker.sock
         │◄───────────────────────────────────────────────│
         │                                                │
         │  User selects display "DP-1" in UI             │
         │  (selection stored in app state)               │
         │                                                │
         │  User clicks Record                            │
         │  App calls portal.CreateSession()              │
         │  App calls portal.SelectSources()              │
         │  App calls portal.Start()                      │
         │                                                │
         │                    Portal routes to picker ───►│
         │                                                │
         │◄─── Picker connects to IPC socket ─────────────│
         │                                                │
         │◄─── { "type": "query_selection" } ─────────────│
         │                                                │
         │──── { "type": "selection",        ────────────►│
         │      "source_type": "monitor",                 │
         │      "source_id": "DP-1",                      │
         │      "geometry": {...} }                       │
         │                                                │
         │     Picker returns approval to portal ────────►│
         │                                                │
         │◄─── Portal returns PipeWire node ID            │
         │                                                │
         │  App connects to PipeWire                      │
         │  Recording begins                              │
         │                                                │
```

## Installation & Setup

Users need to:

1. Install the picker binary:
   ```bash
   # Build and install
   cargo build --release -p screen-recorder-picker
   sudo cp target/release/screen-recorder-picker /usr/local/bin/
   ```

2. Install portal registration:
   ```bash
   sudo cp resources/linux/screen-recorder.portal /usr/share/xdg-desktop-portal/portals/
   ```

3. Configure portal routing:
   ```bash
   mkdir -p ~/.config/xdg-desktop-portal
   cp resources/linux/hyprland-portals.conf ~/.config/xdg-desktop-portal/
   ```

4. Enable and start the picker service:
   ```bash
   cp resources/linux/screen-recorder-picker.service ~/.config/systemd/user/
   systemctl --user daemon-reload
   systemctl --user enable --now screen-recorder-picker
   ```

5. Restart xdg-desktop-portal:
   ```bash
   systemctl --user restart xdg-desktop-portal
   ```

## Open Questions

1. **Fallback behavior**: What should picker do if main app isn't running? Currently: deny request. Future: could show minimal fallback UI or use system picker.

2. **Multiple app instances**: What if user runs multiple screen-recorder instances? Need to handle or prevent.

3. **Audio source selection**: How to present audio sources from PipeWire in the main app UI?

4. **Hyprland version**: What minimum Hyprland version should we target? IPC API has evolved.
