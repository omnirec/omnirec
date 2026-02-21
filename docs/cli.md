# OmniRec Command Line Interface

OmniRec includes a command-line interface (`omnirec`) for headless recording and automation. The CLI communicates with the OmniRec Tauri application via IPC socket to control screen recording without requiring the GUI.

## Installation

The CLI is included with OmniRec installations. After installing OmniRec, the `omnirec` command should be available in your system PATH.

## How It Works

When you run a CLI command, it attempts to connect to a running OmniRec app via IPC socket. If the app is not running, the CLI automatically launches it in headless mode (tray-only, no main window) and waits for it to become ready.

### Headless Mode

The OmniRec Tauri app supports a `--headless` flag that starts the app without a main window, running only in the system tray. This is useful for:

- **CLI automation**: The CLI uses this automatically when spawning the app
- **Background recording**: Start the app for recording without the GUI
- **Server-like operation**: Run OmniRec as a background service

```bash
# Start OmniRec in headless mode (macOS)
open -a OmniRec --args --headless

# Start OmniRec in headless mode (Linux/Windows)
omnirec-app --headless
```

## Quick Start

### Windows, macOS, and Linux with Hyprland

These platforms support full capture source selection, allowing you to specify exactly which window, display, or region to record.

```bash
# List available capture sources
omnirec list windows
omnirec list displays
omnirec list audio

# Record a window (use handle from 'list windows')
omnirec record window 12345

# Record a display (use ID from 'list displays')
omnirec record display 0

# Record a screen region
omnirec record region --display 0 --x 100 --y 100 --width 800 --height 600

# Stop recording (or press Ctrl+C)
omnirec stop

# Check recording status
omnirec status
```

### Other Desktop Environments (GNOME, KDE Plasma, COSMIC, Cinnamon)

On portal-based Wayland desktops, screen capture requires using the system's native picker dialog. Direct window/display/region selection is not available due to Wayland security restrictions.

```bash
# List available audio sources
omnirec list audio

# Start recording (opens native screen picker)
omnirec record portal

# With recording options
omnirec record portal --format webm --duration 60

# Stop recording (or press Ctrl+C)
omnirec stop

# Check recording status
omnirec status
```

> **Note:** If you use `record window`, `record display`, or `record region` on these desktops, the CLI will warn and automatically fall back to portal mode. Use `--strict` to fail instead of falling back.

## Global Options

These options can be used with any command:

| Option | Short | Description |
|--------|-------|-------------|
| `--json` | | Output in JSON format for scripting |
| `--quiet` | `-q` | Suppress non-essential output |
| `--verbose` | `-v` | Enable verbose output |
| `--version` | | Show version information |
| `--help` | `-h` | Show help information |

## Commands

### `list`

List available capture sources.

#### `list windows`

List all capturable windows with their handles, process names, and titles.

```bash
omnirec list windows
omnirec list windows --json
```

**Output columns:**
- `HANDLE` - Window handle (use this value with `record window`)
- `PROCESS` - Process name that owns the window
- `TITLE` - Window title

#### `list displays`

List all available displays/monitors.

```bash
omnirec list displays
omnirec list displays --json
```

**Output columns:**
- `ID` - Display identifier (use this value with `record display` or `--display`)
- `NAME` - Display name
- `RESOLUTION` - Display resolution (width x height)
- `POSITION` - Display position (x, y coordinates)
- `PRIMARY` - Whether this is the primary display

#### `list audio`

List available audio sources (system audio outputs and microphone inputs).

```bash
omnirec list audio
omnirec list audio --json
```

**Output sections:**
- **System Audio** - Audio outputs (use with `--audio`)
- **Microphones** - Audio inputs (use with `--microphone`)

### `record`

Start a recording. Press `Ctrl+C` to stop, or use `omnirec stop` from another terminal.

#### Recording Options

All `record` subcommands accept these options:

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--output <path>` | `-o` | Auto-generated | Output file path |
| `--format <fmt>` | `-f` | `mp4` | Output format: `mp4`, `webm`, `mkv`, `mov`, `gif`, `apng`, `webp` |
| `--duration <secs>` | `-d` | None | Auto-stop after specified seconds |
| `--audio <id>` | | Default | System audio source ID, or `none` to disable |
| `--microphone <id>` | | Default | Microphone source ID, or `none` to disable |
| `--strict` | | false | Fail if specific target cannot be selected (don't fall back to portal) |

#### `record window <handle>`

Record a specific window by its handle.

```bash
# Record window with handle 12345
omnirec record window 12345

# Record with custom format and duration
omnirec record window 12345 --format webm --duration 60

# Record without audio
omnirec record window 12345 --audio none --microphone none
```

**Arguments:**
- `<handle>` - Window handle (get from `omnirec list windows`)

#### `record display <id>`

Record a specific display/monitor.

```bash
# Record display with ID "0"
omnirec record display 0

# Record display "HDMI-1" with specific audio source
omnirec record display HDMI-1 --audio default --microphone usb-mic-1
```

**Arguments:**
- `<id>` - Display ID (get from `omnirec list displays`)

#### `record region`

Record a specific rectangular region of a display.

```bash
omnirec record region --display 0 --x 100 --y 100 --width 800 --height 600
```

**Required options:**
- `--display <id>` - Display ID containing the region
- `--x <pixels>` - X coordinate of the region's top-left corner
- `--y <pixels>` - Y coordinate of the region's top-left corner  
- `--width <pixels>` - Width of the region
- `--height <pixels>` - Height of the region

#### `record portal`

Record using the desktop portal picker (Linux Wayland only). This opens the native screen picker dialog provided by your desktop environment.

```bash
omnirec record portal
omnirec record portal --format webm --duration 120
```

### `stop`

Stop the current recording and save the file.

```bash
omnirec stop
omnirec stop --json
```

### `status`

Show the current recording status.

```bash
omnirec status
omnirec status --json
```

**States:**
- `idle` - No recording in progress
- `recording` - Recording is active
- `saving` - Recording is being saved/transcoded

### `version`

Show version information.

```bash
omnirec version
omnirec version --json
```

## JSON Output

Use `--json` for machine-readable output suitable for scripting:

```bash
# List displays as JSON
omnirec list displays --json

# Get status as JSON
omnirec status --json
```

**Example JSON outputs:**

```json
// omnirec status --json (when recording)
{"state": "recording", "elapsed_seconds": 45}

// omnirec status --json (when idle)
{"state": "idle"}

// omnirec stop --json
{"status": "stopped", "file_path": "/home/user/Videos/recording-2025-01-03.mp4"}
```

## Platform-Specific Behavior

### Windows & macOS

All recording modes (`window`, `display`, `region`) work as expected with direct source selection.

### Linux - Hyprland

Full support for all recording modes with direct source selection. On first use, OmniRec displays an approval dialog. Check "Always allow" to bypass future prompts.

### Linux - GNOME, KDE Plasma, COSMIC, Cinnamon (Wayland)

Due to Wayland security restrictions, specific window/display/region selection is not supported on these desktop environments. The CLI will:

1. **Default behavior:** Warn and fall back to portal mode (opens native picker)
2. **With `--strict`:** Fail with exit code 7 instead of falling back

```bash
# Falls back to portal with a warning
omnirec record window 12345

# Fails with exit code 7
omnirec record window 12345 --strict

# Use portal mode directly (recommended)
omnirec record portal
```

## Exit Codes

| Code | Name | Description |
|------|------|-------------|
| 0 | Success | Operation completed successfully |
| 1 | General Error | Unspecified error |
| 2 | Invalid Arguments | Invalid command-line arguments |
| 3 | App Connection Failed | Failed to connect to the OmniRec app |
| 4 | Recording Failed to Start | Recording could not be started |
| 5 | Recording Failed During Capture | Recording failed while in progress |
| 6 | Transcoding Failed | Output format conversion failed (original MP4 preserved) |
| 7 | Portal Required | Specific target selection unavailable with `--strict` flag |
| 8 | User Cancelled | User cancelled the portal picker dialog |

## Examples

### Basic Recording Workflow

```bash
# 1. Find available windows
omnirec list windows

# 2. Start recording a specific window
omnirec record window 12345

# 3. Stop recording when done (or press Ctrl+C)
omnirec stop
```

### Scripted Recording

```bash
#!/bin/bash
# Record for 60 seconds and save as WebM

omnirec record display 0 \
    --format webm \
    --duration 60 \
    --output ~/Videos/automated-recording.webm

if [ $? -eq 0 ]; then
    echo "Recording saved successfully"
else
    echo "Recording failed with exit code $?"
fi
```

### Recording with Audio Configuration

```bash
# List available audio sources
omnirec list audio

# Record with specific system audio and microphone
omnirec record display 0 --audio default --microphone usb-mic-1

# Record system audio only (no microphone)
omnirec record window 12345 --audio default --microphone none

# Record with no audio at all
omnirec record display 0 --audio none --microphone none
```

### Monitoring Recording Status

```bash
# Check if recording is in progress
STATUS=$(omnirec status --json)
echo $STATUS | jq '.state'

# Get elapsed time during recording
omnirec status --json | jq '.elapsed_seconds'
```
