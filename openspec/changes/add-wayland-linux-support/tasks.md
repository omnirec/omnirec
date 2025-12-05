# Tasks: Add Wayland/Linux Support (Hyprland)

## Phase 1: Picker + IPC + Display Capture

### 1.1 Platform Abstraction Layer (COMPLETED)
- [x] 1.1.1 Create `CaptureBackend` trait in `src-tauri/src/capture/mod.rs`
- [x] 1.1.2 Move Windows code to `src-tauri/src/capture/windows/` module
- [x] 1.1.3 Implement `WindowsBackend` wrapping existing code
- [x] 1.1.4 Add conditional compilation gates
- [x] 1.1.5 Update `Cargo.toml` with platform-specific dependencies
- [x] 1.1.6 Create Linux stub backend
- [x] 1.1.7 Create macOS stub backend

### 1.2 Custom Picker for XDPH (COMPLETED - Simplified)
The picker is now a simple stdout-based executable that xdg-desktop-portal-hyprland
invokes instead of hyprland-share-picker. It queries the main app via IPC and outputs
the selection in the format XDPH expects.

- [x] 1.2.1 Create `src-picker/` crate with Cargo.toml
- [x] 1.2.2 Implement IPC client (Unix socket connection to main app)
- [x] 1.2.3 Query main app for current selection
- [x] 1.2.4 Output selection to stdout in XDPH format:
  - Screen: `[SELECTION]/screen:<output_name>`
  - Window: `[SELECTION]/window:<window_handle>`
  - Region: `[SELECTION]/region:<output>@<x>,<y>,<w>,<h>`
- [x] 1.2.5 Handle "main app not running" case (exit with error, XDPH cancels request)

### 1.3 IPC Protocol (COMPLETED)
- [x] 1.3.1 Define IPC message types (JSON-based)
  - [x] QuerySelection request
  - [x] Selection response (source_type, source_id, geometry)
  - [x] NoSelection response
- [x] 1.3.2 Implement IPC server in main app (`src-tauri/src/capture/linux/ipc_server.rs`)
- [x] 1.3.3 Implement IPC client in picker (`src-picker/src/ipc_client.rs`)
- [x] 1.3.4 Handle connection/reconnection logic
- [x] 1.3.5 Socket path: `$XDG_RUNTIME_DIR/screen-recorder/picker.sock`

### 1.4 Hyprland Monitor Enumeration (COMPLETED)
- [x] 1.4.1 Create Hyprland IPC in `src-tauri/src/capture/linux/mod.rs` (using hyprland crate)
- [x] 1.4.2 Implement Hyprland IPC socket connection (via hyprland crate)
- [x] 1.4.3 Implement `list_monitors()` via Hyprland crate's `Monitors::get()`
- [x] 1.4.4 Map Hyprland monitor data to `MonitorInfo` struct
- [x] 1.4.5 Implement `MonitorEnumerator` trait for `LinuxBackend`
- [x] 1.4.6 Handle Hyprland not running (error)

### 1.5 Portal Client (Main App) (COMPLETED)
- [x] 1.5.1 Add `ashpd` crate dependency
- [x] 1.5.2 Create `portal_client.rs` with basic portal flow
- [x] 1.5.3 Integrate portal client with IPC server (store selection before request)
- [x] 1.5.4 Handle portal response and extract PipeWire node ID

### 1.6 Installation Files (COMPLETED - Simplified)
With the new XDPH-based approach, only one config file is needed:

- [x] 1.6.1 Create `resources/linux/xdph.conf` (XDPH config to use our picker)
- [ ] 1.6.2 Document installation steps in README

Old files removed (no longer needed):
- ~~screen-recorder.portal~~ (we use XDPH, not our own portal backend)
- ~~screen-recorder-picker.service~~ (picker is invoked by XDPH, not a daemon)
- ~~hyprland-portals.conf~~ (we don't override portal routing)

### 1.7 Integration Testing
- [x] 1.7.1 Test IPC connection between app and picker
- [ ] 1.7.2 Test XDPH invokes our picker when configured
- [ ] 1.7.3 Test picker outputs correct format for XDPH
- [ ] 1.7.4 Verify recording starts without picker UI appearing

## Phase 2: PipeWire Capture + Window Support

### 2.1 PipeWire Video Capture
- [x] 2.1.1 Add `pipewire` crate dependency
- [x] 2.1.2 Create `src-tauri/src/capture/linux/pipewire_capture.rs`
- [x] 2.1.3 Implement PipeWire stream connection from node ID
- [x] 2.1.4 Implement frame buffer handling (SHM and DMA-BUF)
- [x] 2.1.5 Convert frames to BGRA format
- [x] 2.1.6 Implement frame channel (FrameReceiver)
- [x] 2.1.7 Handle stream errors and disconnection

### 2.2 Display Recording Integration
- [ ] 2.2.1 Implement `start_display_capture()` for LinuxBackend
- [ ] 2.2.2 Wire up selection → IPC → portal → PipeWire flow
- [ ] 2.2.3 Test full display recording on Hyprland
- [ ] 2.2.4 Verify output video quality and framerate

### 2.3 Hyprland Window Enumeration (COMPLETED in 1.4)
- [x] 2.3.1 Implement `list_windows()` via Hyprland crate's `Clients::get()`
- [x] 2.3.2 Map Hyprland client data to `WindowInfo` struct
- [x] 2.3.3 Filter out hidden/special windows
- [x] 2.3.4 Implement `WindowEnumerator` trait for LinuxBackend
- [x] 2.3.5 Map window address to usable ID for portal

### 2.4 Window Capture Integration
- [x] 2.4.1 Extend IPC protocol for window selection (done in 1.3)
- [x] 2.4.2 Update picker to handle window source type
- [ ] 2.4.3 Implement `start_window_capture()` for LinuxBackend
- [ ] 2.4.4 Handle window resize during capture
- [ ] 2.4.5 Handle window close during capture
- [ ] 2.4.6 Test full window recording on Hyprland

## Phase 3: Region Capture

### 3.1 Region Selection
- [ ] 3.1.1 Adapt selection overlay for Wayland/Tauri
- [ ] 3.1.2 Get region bounds relative to monitor
- [ ] 3.1.3 Store region in app state for IPC

### 3.2 Region Capture Implementation
- [x] 3.2.1 Extend IPC protocol for region selection (monitor + bounds) - done in 1.3
- [x] 3.2.2 Update picker to output region format for XDPH
- [ ] 3.2.3 Capture full monitor via portal
- [ ] 3.2.4 Implement region cropping from monitor stream
- [ ] 3.2.5 Handle region boundary validation
- [ ] 3.2.6 Test region recording on Hyprland

## Phase 4: Audio Capture

### 4.1 PipeWire Audio Integration
- [ ] 4.1.1 Extend portal request to include audio sources
- [ ] 4.1.2 Implement audio stream handling in PipeWire capture
- [ ] 4.1.3 Create audio sample buffer and channel

### 4.2 Audio Encoding
- [ ] 4.2.1 Add audio encoding to FFmpeg pipeline
- [ ] 4.2.2 Implement audio-video timestamp synchronization
- [ ] 4.2.3 Test muxed output file

## Phase 5: Polish & Error Handling

### 5.1 Error Handling
- [ ] 5.1.1 Handle XDPH not installed/configured
- [ ] 5.1.2 Handle main app not running (picker side) - exits with error
- [ ] 5.1.3 Handle IPC connection failures
- [ ] 5.1.4 Handle PipeWire disconnection
- [ ] 5.1.5 Handle Hyprland IPC failures
- [ ] 5.1.6 User-friendly error messages

### 5.2 Performance Optimization
- [ ] 5.2.1 Investigate DMA-BUF support for zero-copy capture
- [ ] 5.2.2 Profile frame processing pipeline
- [ ] 5.2.3 Optimize buffer allocations

### 5.3 Documentation & Installation
- [ ] 5.3.1 Document Linux build requirements
- [ ] 5.3.2 Document installation steps (picker binary + XDPH config)
- [ ] 5.3.3 Document Hyprland version requirements

## Validation Checkpoints

- [x] **V0 (POC)**: Portal client can request screencast (with system picker UI)
- [x] **V1**: IPC works between main app and picker
- [x] **V2**: Picker outputs correct format for XDPH
- [ ] **V3**: Display capture works end-to-end (no picker UI) with XDPH integration
- [ ] **V4**: Window capture works with Hyprland enumeration
- [ ] **V5**: Region capture works with overlay
- [ ] **V6**: Audio capture works and syncs with video
- [ ] **V7**: All error paths handled gracefully

## Architecture Summary

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

## Installation

1. Build and install the picker:
   ```bash
   cargo build --release -p screen-recorder-picker
   sudo cp target/release/screen-recorder-picker /usr/local/bin/
   ```

2. Configure XDPH to use our picker:
   ```bash
   cp resources/linux/xdph.conf ~/.config/hypr/xdph.conf
   ```

3. Restart XDPH:
   ```bash
   systemctl --user restart xdg-desktop-portal-hyprland
   ```

## Notes

- The picker is **invoked by XDPH**, not a long-running daemon
- XDPH handles all PipeWire/Wayland complexity - we just provide selection
- Main app uses **Hyprland IPC directly** for enumeration (no portal)
- Main app uses **portal** (via ashpd) for capture authorization
- If main app not running, picker exits with error → XDPH cancels request
