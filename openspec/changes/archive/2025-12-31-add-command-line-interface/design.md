# Design: Command-Line Interface

## Context

OmniRec uses a client-server architecture where `omnirec-service` handles all capture and encoding operations, communicating via IPC (Unix sockets on Linux/macOS, named pipes on Windows). The Tauri GUI is a thin client that sends IPC requests to the service.

The CLI will be another thin client using the same IPC protocol, enabling headless recording without the GUI.

### Constraints

- Must work alongside or independently of the GUI client
- Must handle Wayland restrictions where programmatic source selection is not possible
- Should reuse existing IPC protocol without modifications
- Must provide appropriate exit codes for scripting

## Goals / Non-Goals

### Goals
- Provide headless recording capability for all capture modes
- Enable scriptable automation with JSON output and exit codes
- Support all existing output formats and audio settings
- Work on all supported platforms (Windows, macOS, Linux)
- Handle Wayland portal restrictions gracefully

### Non-Goals
- Real-time streaming (recording to file only)
- Interactive TUI with live preview
- Modifying the IPC protocol or service architecture
- GUI-equivalent feature parity (no thumbnail preview, no highlight visualization)

## Decisions

### Decision: Separate CLI Binary

The CLI will be a standalone binary (`omnirec` or `omnirec.exe`) rather than being embedded in the service or GUI.

**Rationale:**
- Keeps binaries focused (single responsibility)
- Allows independent versioning if needed
- Smaller binary size for CLI-only deployments
- Consistent with Unix philosophy

**Alternatives considered:**
- Service subcommand: Would require service to handle both server mode and CLI commands, increasing complexity
- GUI flag: Would require loading GUI dependencies even when not needed

### Decision: Clap for Argument Parsing

Use the `clap` crate with derive macros for argument parsing.

**Rationale:**
- Industry standard for Rust CLI applications
- Automatic help generation
- Shell completion generation
- Type-safe argument parsing

### Decision: Service Management

The CLI will start the service automatically if not running and connect to existing service if already running.

**Rationale:**
- Matches GUI behavior
- Single recording session at a time (enforced by service)
- Service handles graceful shutdown and file finalization

### Decision: Wayland Portal Handling

On portal-mode desktops, the CLI will:
1. For `record portal`: Trigger portal picker dialog, record whatever user selects
2. For `record window/display/region`: Warn that target cannot be pre-selected, fall back to portal

**Rationale:**
- Wayland security model prevents programmatic source selection
- Portal picker is the only way to initiate capture on GNOME/KDE/COSMIC/Cinnamon
- Providing a `portal` subcommand gives explicit control
- Clear error messaging prevents user confusion

### Decision: Event Subscription for Progress

The CLI will subscribe to service events for real-time status updates (elapsed time, transcoding progress).

**Rationale:**
- Provides feedback during long recordings
- Enables proper handling of async operations (transcoding)
- Reuses existing event streaming infrastructure

### Decision: JSON Output Mode

A `--json` flag enables machine-readable output for all commands.

**Rationale:**
- Essential for scripting and automation
- Allows integration with other tools (jq, scripts)
- Standard practice for modern CLIs

## Architecture

```
omnirec (CLI)          omnirec-service
    |                       |
    |--- IPC connect ------>|
    |                       |
    |--- list_windows ----->|
    |<-- windows response --|
    |                       |
    |--- start_capture ---->|
    |<-- recording_started -|
    |                       |
    |--- subscribe_events ->|
    |<-- event stream ------|
    |                       |
    |--- stop_recording --->|
    |<-- recording_stopped -|
```

## Command Structure

```
omnirec
  list
    windows           List capturable windows
    displays          List available displays/monitors
    audio             List audio sources (system + microphone)
  record
    window <HANDLE>   Record specific window by handle
    display <ID>      Record specific display by ID
    region            Record region (with coordinates)
    portal            Record using portal picker (Wayland)
  stop                Stop current recording
  status              Show current recording state
  version             Show version information
```

### Common Flags

```
--output, -o <PATH>   Output file path (overrides config)
--format, -f <FMT>    Output format (mp4, webm, mkv, mov, gif, apng, webp)
--duration, -d <SEC>  Auto-stop after duration
--audio <ID>          System audio source ID (or "none")
--microphone <ID>     Microphone source ID (or "none")
--json                Output in JSON format
--quiet, -q           Suppress non-essential output
--verbose, -v         Verbose output
```

### Region-Specific Flags

```
record region
  --display <ID>      Target display for region
  --x <INT>           X coordinate (pixels)
  --y <INT>           Y coordinate (pixels)
  --width <INT>       Width (pixels)
  --height <INT>      Height (pixels)
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0    | Success |
| 1    | General error |
| 2    | Invalid arguments |
| 3    | Service connection failed |
| 4    | Recording failed to start |
| 5    | Recording failed during capture |
| 6    | Transcoding failed |
| 7    | Portal required (Wayland restriction) |
| 8    | User cancelled (portal picker) |

## Risks / Trade-offs

### Risk: Long-running CLI sessions
**Concern:** CLI process must stay running for duration of recording.
**Mitigation:** Clear documentation; suggest using screen/tmux for remote sessions. Duration flag auto-stops recordings.

### Risk: Signal handling complexity
**Concern:** Ctrl+C must gracefully stop recording and save file.
**Mitigation:** Handle SIGINT/SIGTERM, send stop command to service, wait for file save before exit.

### Risk: Service not available
**Concern:** Service may fail to start or crash during recording.
**Mitigation:** Health check on connect, reconnection attempts, clear error messages.

## Migration Plan

No migration required. This is a new additive capability that doesn't affect existing functionality.

## Open Questions

None - the design leverages existing IPC infrastructure and follows established patterns.
