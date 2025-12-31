# command-line-interface Specification

## Purpose

Provides headless recording capability via command-line interface, enabling scriptable automation, remote workflows, and use cases where a GUI is unavailable.

## ADDED Requirements

### Requirement: CLI Binary

The system SHALL provide a standalone command-line binary (`omnirec` on Unix, `omnirec.exe` on Windows) for headless recording operations.

#### Scenario: CLI binary available

- **WHEN** OmniRec is installed
- **THEN** the `omnirec` CLI binary SHALL be available in the installation directory
- **AND** the binary SHALL be executable from the command line

#### Scenario: CLI connects to service

- **WHEN** any CLI command is executed
- **THEN** the CLI SHALL connect to the running `omnirec-service`
- **OR** start the service automatically if not running
- **AND** communicate via the existing IPC protocol

#### Scenario: CLI works independently of GUI

- **WHEN** the CLI is used
- **THEN** the Tauri GUI SHALL NOT be required
- **AND** recording can be performed entirely from the command line

### Requirement: List Windows Command

The CLI SHALL provide a command to enumerate available windows for capture.

#### Scenario: List windows successfully

- **WHEN** the user executes `omnirec list windows`
- **THEN** the CLI SHALL display a list of capturable windows
- **AND** each entry SHALL include the window handle, title, and process name

#### Scenario: List windows JSON output

- **WHEN** the user executes `omnirec list windows --json`
- **THEN** the CLI SHALL output the window list as JSON
- **AND** the JSON SHALL include handle, title, and process_name fields

#### Scenario: No windows available

- **WHEN** the user executes `omnirec list windows`
- **AND** no capturable windows exist
- **THEN** the CLI SHALL display a message indicating no windows are available
- **AND** exit with code 0

### Requirement: List Displays Command

The CLI SHALL provide a command to enumerate available displays/monitors for capture.

#### Scenario: List displays successfully

- **WHEN** the user executes `omnirec list displays`
- **THEN** the CLI SHALL display a list of available monitors
- **AND** each entry SHALL include the monitor ID, name, resolution, position, and primary status

#### Scenario: List displays JSON output

- **WHEN** the user executes `omnirec list displays --json`
- **THEN** the CLI SHALL output the display list as JSON
- **AND** the JSON SHALL include id, name, width, height, x, y, and is_primary fields

### Requirement: List Audio Command

The CLI SHALL provide a command to enumerate available audio sources.

#### Scenario: List audio sources successfully

- **WHEN** the user executes `omnirec list audio`
- **THEN** the CLI SHALL display available audio output devices (system audio)
- **AND** available audio input devices (microphones)
- **AND** each entry SHALL include the source ID, name, and type

#### Scenario: List audio JSON output

- **WHEN** the user executes `omnirec list audio --json`
- **THEN** the CLI SHALL output the audio source list as JSON
- **AND** the JSON SHALL include id, name, and source_type fields

### Requirement: Record Window Command

The CLI SHALL provide a command to record a specific window by handle.

#### Scenario: Record window successfully

- **WHEN** the user executes `omnirec record window <HANDLE>`
- **AND** the handle refers to a valid, capturable window
- **THEN** recording SHALL begin for the specified window
- **AND** the CLI SHALL display "Recording started..."
- **AND** the CLI SHALL continue running until recording is stopped

#### Scenario: Record window with invalid handle

- **WHEN** the user executes `omnirec record window <HANDLE>`
- **AND** the handle does not refer to a valid window
- **THEN** the CLI SHALL display an error message
- **AND** exit with code 4 (recording failed to start)

#### Scenario: Record window on portal-mode desktop

- **WHEN** the user executes `omnirec record window <HANDLE>` on a portal-mode desktop
- **THEN** the CLI SHALL display a warning that window selection is not supported
- **AND** the CLI SHALL fall back to portal-based recording
- **OR** exit with code 7 (portal required) if `--strict` flag is used

### Requirement: Record Display Command

The CLI SHALL provide a command to record a specific display/monitor by ID.

#### Scenario: Record display successfully

- **WHEN** the user executes `omnirec record display <ID>`
- **AND** the ID refers to a valid, available display
- **THEN** recording SHALL begin for the entire display
- **AND** the CLI SHALL display "Recording started..."

#### Scenario: Record display with invalid ID

- **WHEN** the user executes `omnirec record display <ID>`
- **AND** the ID does not refer to a valid display
- **THEN** the CLI SHALL display an error message
- **AND** exit with code 4

#### Scenario: Record display on portal-mode desktop

- **WHEN** the user executes `omnirec record display <ID>` on a portal-mode desktop
- **THEN** the CLI SHALL display a warning that display selection is not supported
- **AND** the CLI SHALL fall back to portal-based recording

### Requirement: Record Region Command

The CLI SHALL provide a command to record a specific screen region.

#### Scenario: Record region with coordinates

- **WHEN** the user executes `omnirec record region --display <ID> --x <X> --y <Y> --width <W> --height <H>`
- **AND** all coordinates are valid for the specified display
- **THEN** recording SHALL begin for the specified region
- **AND** the CLI SHALL display "Recording started..."

#### Scenario: Record region missing coordinates

- **WHEN** the user executes `omnirec record region` without all required coordinate flags
- **THEN** the CLI SHALL display an error indicating missing required flags
- **AND** exit with code 2 (invalid arguments)

#### Scenario: Record region with invalid coordinates

- **WHEN** the user executes `omnirec record region` with coordinates outside display bounds
- **THEN** the CLI SHALL display an error message
- **AND** exit with code 2

#### Scenario: Record region on portal-mode desktop

- **WHEN** the user executes `omnirec record region` on a portal-mode desktop
- **THEN** the CLI SHALL display a warning that region selection is not supported
- **AND** the CLI SHALL fall back to portal-based recording

### Requirement: Record Portal Command

The CLI SHALL provide a command to initiate recording via the desktop portal picker.

#### Scenario: Record via portal

- **WHEN** the user executes `omnirec record portal`
- **THEN** the desktop's native screen sharing picker SHALL appear
- **AND** recording SHALL begin when the user makes a selection
- **AND** the CLI SHALL display "Waiting for portal selection..."

#### Scenario: Portal selection cancelled

- **WHEN** the user executes `omnirec record portal`
- **AND** the user cancels the portal picker dialog
- **THEN** the CLI SHALL display "Recording cancelled"
- **AND** exit with code 8 (user cancelled)

#### Scenario: Portal not available

- **WHEN** the user executes `omnirec record portal`
- **AND** the platform does not support portal-based capture (e.g., Windows, Hyprland)
- **THEN** the CLI SHALL display an error that portal is not supported on this platform
- **AND** exit with code 1

### Requirement: Recording Output Configuration

The CLI SHALL support configuring output file path and format via command-line flags.

#### Scenario: Custom output path

- **WHEN** the user executes a record command with `--output <PATH>`
- **THEN** the recording SHALL be saved to the specified path
- **AND** the directory SHALL be created if it does not exist

#### Scenario: Custom output format

- **WHEN** the user executes a record command with `--format <FORMAT>`
- **AND** FORMAT is one of: mp4, webm, mkv, mov, gif, apng, webp
- **THEN** the recording SHALL be transcoded to the specified format

#### Scenario: Invalid output format

- **WHEN** the user executes a record command with `--format <FORMAT>`
- **AND** FORMAT is not a valid output format
- **THEN** the CLI SHALL display an error listing valid formats
- **AND** exit with code 2

#### Scenario: Default output location

- **WHEN** the user executes a record command without `--output`
- **THEN** the recording SHALL be saved to the configured output directory
- **OR** the system default Videos folder if not configured

### Requirement: Recording Duration Limit

The CLI SHALL support automatic recording termination after a specified duration.

#### Scenario: Recording with duration limit

- **WHEN** the user executes a record command with `--duration <SECONDS>`
- **THEN** recording SHALL automatically stop after the specified seconds
- **AND** the file SHALL be saved normally
- **AND** the CLI SHALL exit with code 0

#### Scenario: Duration with manual stop

- **WHEN** the user executes a record command with `--duration <SECONDS>`
- **AND** the user stops recording before the duration (Ctrl+C or `omnirec stop`)
- **THEN** recording SHALL stop immediately
- **AND** the file SHALL be saved normally

### Requirement: Audio Configuration

The CLI SHALL support configuring audio capture via command-line flags.

#### Scenario: Record with system audio

- **WHEN** the user executes a record command with `--audio <ID>`
- **AND** ID refers to a valid audio output device
- **THEN** system audio SHALL be captured from the specified device

#### Scenario: Record with microphone

- **WHEN** the user executes a record command with `--microphone <ID>`
- **AND** ID refers to a valid audio input device
- **THEN** microphone audio SHALL be captured from the specified device

#### Scenario: Disable audio

- **WHEN** the user executes a record command with `--audio none`
- **THEN** no system audio SHALL be captured

#### Scenario: Disable microphone

- **WHEN** the user executes a record command with `--microphone none`
- **THEN** no microphone audio SHALL be captured

### Requirement: Stop Command

The CLI SHALL provide a command to stop an in-progress recording.

#### Scenario: Stop recording

- **WHEN** a recording is in progress
- **AND** the user executes `omnirec stop`
- **THEN** the recording SHALL be stopped
- **AND** the file SHALL be saved
- **AND** the CLI SHALL display the saved file path

#### Scenario: Stop when not recording

- **WHEN** no recording is in progress
- **AND** the user executes `omnirec stop`
- **THEN** the CLI SHALL display "No recording in progress"
- **AND** exit with code 0

### Requirement: Status Command

The CLI SHALL provide a command to query the current recording state.

#### Scenario: Status when recording

- **WHEN** a recording is in progress
- **AND** the user executes `omnirec status`
- **THEN** the CLI SHALL display "Recording" and the elapsed time

#### Scenario: Status when idle

- **WHEN** no recording is in progress
- **AND** the user executes `omnirec status`
- **THEN** the CLI SHALL display "Idle"

#### Scenario: Status JSON output

- **WHEN** the user executes `omnirec status --json`
- **THEN** the CLI SHALL output the status as JSON
- **AND** the JSON SHALL include state and elapsed_seconds fields

### Requirement: Signal Handling

The CLI SHALL handle system signals gracefully during recording.

#### Scenario: Ctrl+C during recording

- **WHEN** a recording is in progress
- **AND** the user presses Ctrl+C
- **THEN** the recording SHALL be stopped gracefully
- **AND** the file SHALL be saved
- **AND** the CLI SHALL wait for the file to be finalized
- **AND** the CLI SHALL exit with code 0

#### Scenario: SIGTERM during recording

- **WHEN** a recording is in progress
- **AND** the process receives SIGTERM
- **THEN** the recording SHALL be stopped gracefully
- **AND** the file SHALL be saved

### Requirement: Exit Codes

The CLI SHALL use structured exit codes for scripting integration.

#### Scenario: Successful operation

- **WHEN** a CLI operation completes successfully
- **THEN** the CLI SHALL exit with code 0

#### Scenario: General error

- **WHEN** an unspecified error occurs
- **THEN** the CLI SHALL exit with code 1

#### Scenario: Invalid arguments

- **WHEN** invalid or missing arguments are provided
- **THEN** the CLI SHALL exit with code 2

#### Scenario: Service connection failed

- **WHEN** the CLI cannot connect to the service
- **THEN** the CLI SHALL exit with code 3

#### Scenario: Recording failed to start

- **WHEN** recording fails to start (invalid target, permissions, etc.)
- **THEN** the CLI SHALL exit with code 4

#### Scenario: Recording failed during capture

- **WHEN** recording fails during capture (target closed, encoder error)
- **THEN** the CLI SHALL exit with code 5

#### Scenario: Transcoding failed

- **WHEN** post-recording transcoding fails
- **THEN** the CLI SHALL exit with code 6
- **AND** the original MP4 SHALL be preserved

#### Scenario: Portal required

- **WHEN** a specific target is requested on a portal-mode desktop with `--strict` flag
- **THEN** the CLI SHALL exit with code 7

#### Scenario: User cancelled

- **WHEN** the user cancels a portal picker dialog
- **THEN** the CLI SHALL exit with code 8

### Requirement: Wayland Portal Detection

The CLI SHALL detect when running on a portal-mode Wayland desktop and adjust behavior accordingly.

#### Scenario: GNOME detected

- **WHEN** the CLI starts on Linux
- **AND** `XDG_CURRENT_DESKTOP` contains "GNOME"
- **THEN** the CLI SHALL operate in portal-mode
- **AND** specific target selection (window, display, region) SHALL warn and fall back to portal

#### Scenario: KDE detected

- **WHEN** the CLI starts on Linux
- **AND** `XDG_CURRENT_DESKTOP` contains "KDE"
- **THEN** the CLI SHALL operate in portal-mode

#### Scenario: COSMIC detected

- **WHEN** the CLI starts on Linux
- **AND** `XDG_CURRENT_DESKTOP` contains "COSMIC"
- **THEN** the CLI SHALL operate in portal-mode

#### Scenario: Cinnamon Wayland detected

- **WHEN** the CLI starts on Linux
- **AND** `XDG_CURRENT_DESKTOP` contains "X-CINNAMON"
- **THEN** the CLI SHALL operate in portal-mode

#### Scenario: Hyprland detected

- **WHEN** the CLI starts on Linux
- **AND** `XDG_CURRENT_DESKTOP` does not contain GNOME, KDE, COSMIC, or X-CINNAMON
- **THEN** the CLI SHALL allow specific target selection
- **AND** `record portal` SHALL return an error (use custom picker workflow)

### Requirement: Progress Output

The CLI SHALL provide progress feedback during recording operations.

#### Scenario: Elapsed time display

- **WHEN** recording is in progress
- **AND** `--quiet` flag is NOT set
- **THEN** the CLI SHALL periodically display the elapsed recording time

#### Scenario: Transcoding progress

- **WHEN** transcoding is in progress after recording
- **THEN** the CLI SHALL display "Transcoding to <format>..."
- **AND** display completion message when done

#### Scenario: Quiet mode

- **WHEN** recording is started with `--quiet` flag
- **THEN** the CLI SHALL NOT display progress updates
- **AND** SHALL only output the final file path on completion

#### Scenario: Verbose mode

- **WHEN** recording is started with `--verbose` flag
- **THEN** the CLI SHALL display detailed status information
- **AND** include service connection status, format details, and timing

### Requirement: Version Command

The CLI SHALL provide a command to display version information.

#### Scenario: Display version

- **WHEN** the user executes `omnirec version` or `omnirec --version`
- **THEN** the CLI SHALL display the application version
- **AND** exit with code 0

### Requirement: Help Text

The CLI SHALL provide comprehensive help text for all commands.

#### Scenario: Display main help

- **WHEN** the user executes `omnirec --help` or `omnirec -h`
- **THEN** the CLI SHALL display usage information
- **AND** list all available subcommands

#### Scenario: Display subcommand help

- **WHEN** the user executes `omnirec <subcommand> --help`
- **THEN** the CLI SHALL display usage information for that subcommand
- **AND** list all available flags and options
