# Change: Refactor to Client-Server Architecture

## Why

The current monolithic Tauri application combines UI, capture, and encoding logic in a single process. Separating capture/encoding into a dedicated background service enables:

1. **Decoupled lifecycle** - Service can run independently, enabling future features like headless recording, scheduled captures, or CLI control
2. **Reduced UI process complexity** - Tauri app becomes a lightweight UI client
3. **Better resource isolation** - Capture/encoding runs in its own process with dedicated resources
4. **Foundation for multi-client support** - Multiple UIs (CLI, web, etc.) could connect to the same service

## What Changes

- **ADDED** New background service application (`omnirec-service`) that runs without visible UI
- **ADDED** IPC interface (Unix domain socket / named pipe) for communication between client and service
- **MODIFIED** Tauri application becomes a UI-only client that communicates via IPC
- **MODIFIED** All capture backends (Windows, Linux, macOS) moved from Tauri to service
- **MODIFIED** All encoding logic (FFmpeg, transcoding) moved from Tauri to service
- **MODIFIED** Audio capture and configuration moved to service
- **MODIFIED** Tauri app starts the service on launch if not already running
- **MODIFIED** Recording state management moved to service (single source of truth)
- **MODIFIED** Configuration loading/saving remains in Tauri but applies via IPC

### Components Remaining in Tauri (Client)

- All UI rendering (HTML/CSS/TypeScript)
- Window/display/region selection UI
- Settings/configuration UI
- About dialog
- System tray functionality (Linux GNOME mode)
- Region selector overlay window
- Service lifecycle management (start on launch)

### Components Moving to Service

- `capture/` - All platform capture backends
- `encoder/` - FFmpeg encoding and transcoding
- `state.rs` - RecordingManager and recording state
- Audio capture and mixing
- FFmpeg initialization and management
- Linux IPC server for picker (already exists, will be extended)

## Impact

- Affected specs: `platform-abstraction`, `recording-control`, `audio-capture`, `wayland-portal`
- Affected code: Major restructuring of `src-tauri/src/`
- New binary: `omnirec-service` (separate Cargo target)
- **BREAKING**: Internal architecture change, but user-facing behavior unchanged
