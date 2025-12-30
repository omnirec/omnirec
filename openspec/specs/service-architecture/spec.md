# service-architecture Specification

## Purpose
TBD - created by archiving change refactor-client-server-architecture. Update Purpose after archive.
## Requirements
### Requirement: Background Service Process

The system SHALL provide a background service process (`omnirec-service`) that runs without visible UI and handles all capture, encoding, and recording operations.

#### Scenario: Service starts without UI

- **WHEN** the service binary is executed
- **THEN** no window or visible UI is displayed
- **AND** the service initializes capture backends for the current platform
- **AND** the service initializes FFmpeg for encoding
- **AND** the service creates an IPC socket for client connections

#### Scenario: Service initializes platform backends

- **WHEN** the service starts
- **THEN** platform-specific capture backends are initialized
- **AND** audio backends are initialized
- **AND** the service is ready to accept capture requests

#### Scenario: Service handles multiple operations

- **WHEN** a client is connected to the service
- **THEN** the service can handle enumeration requests (windows, monitors, audio)
- **AND** the service can handle capture operations (thumbnails, highlights)
- **AND** the service can handle recording operations (start, stop)
- **AND** operations can be performed concurrently where appropriate

### Requirement: Service Lifecycle Management

The Tauri client SHALL start the service automatically when launched and manage its lifecycle.

#### Scenario: Service started on client launch

- **WHEN** the Tauri application starts
- **AND** the service is not already running
- **THEN** the client spawns the service process
- **AND** the client waits for the service to create its IPC socket
- **AND** the client connects to the service

#### Scenario: Service already running

- **WHEN** the Tauri application starts
- **AND** the service is already running
- **THEN** the client connects to the existing service instance
- **AND** no new service process is spawned

#### Scenario: Service crash handling

- **WHEN** the service process crashes during client operation
- **THEN** the client detects the disconnection
- **AND** an error is displayed to the user
- **AND** the client returns to a disconnected state
- **AND** the client can attempt to restart the service

### Requirement: Recording State Ownership

The service SHALL be the single source of truth for all recording state.

#### Scenario: Client queries recording state

- **WHEN** the client needs the current recording state
- **THEN** the client sends a state query to the service
- **AND** the service responds with the current state (idle, recording, saving)

#### Scenario: Recording state changes

- **WHEN** recording is started or stopped via the service
- **THEN** the service updates its internal state
- **AND** the service emits a state change event to subscribed clients

#### Scenario: Elapsed time tracking

- **WHEN** a recording is in progress
- **THEN** the service tracks elapsed time
- **AND** the client can query elapsed time via IPC
- **AND** the service can stream elapsed time updates to subscribed clients

### Requirement: Cross-Platform Socket Path

The service SHALL use platform-appropriate IPC transport with well-defined paths.

#### Scenario: Unix socket on Linux

- **WHEN** the service runs on Linux
- **THEN** the IPC socket is created at `$XDG_RUNTIME_DIR/omnirec/service.sock`
- **AND** the socket has appropriate permissions (owner only)

#### Scenario: Unix socket on macOS

- **WHEN** the service runs on macOS
- **THEN** the IPC socket is created at `$TMPDIR/omnirec/service.sock`
- **AND** the socket has appropriate permissions (owner only)

#### Scenario: Named pipe on Windows

- **WHEN** the service runs on Windows
- **THEN** a named pipe is created at `\\.\pipe\omnirec-service`
- **AND** the pipe has a security descriptor restricting access to the current user
- **AND** the pipe accepts connections from trusted OmniRec executables only

#### Scenario: Windows named pipe server accepts connections

- **WHEN** the service is running on Windows
- **AND** a client connects to the named pipe
- **THEN** the service verifies the client process using GetNamedPipeClientProcessId
- **AND** the service retrieves the client executable path using QueryFullProcessImageNameW
- **AND** the service validates the executable is a trusted OmniRec binary
- **AND** connections from untrusted executables are rejected

#### Scenario: Windows named pipe server cleanup

- **WHEN** the service shuts down on Windows
- **THEN** the named pipe handle is closed
- **AND** any active client connections are terminated gracefully

### Requirement: Graceful Shutdown

The service SHALL shutdown gracefully when requested or when the last client disconnects.

#### Scenario: Client requests shutdown

- **WHEN** a client sends a shutdown request
- **THEN** the service stops any active recording
- **AND** the service finalizes any pending encodes
- **AND** the service closes all client connections
- **AND** the service process exits cleanly

#### Scenario: Last client disconnects

- **WHEN** the last connected client disconnects
- **AND** no recording is in progress
- **THEN** the service may optionally exit
- **OR** the service may continue running for quick reconnection

#### Scenario: Recording in progress on shutdown

- **WHEN** a shutdown is requested
- **AND** a recording is in progress
- **THEN** the service stops the recording
- **AND** the service saves the partial recording
- **AND** the service notifies connected clients of the forced stop

### Requirement: Windows Service Lifecycle

The service SHALL operate correctly on Windows with proper signal handling and process management.

#### Scenario: Service starts on Windows

- **WHEN** the service binary is executed on Windows
- **THEN** no console window is displayed (if run as background process)
- **AND** the service initializes the Windows capture backend
- **AND** the service creates the named pipe for IPC
- **AND** the service is ready to accept client connections

#### Scenario: Ctrl+C handling on Windows

- **WHEN** the service receives Ctrl+C on Windows
- **THEN** the service initiates graceful shutdown
- **AND** active recordings are stopped and saved
- **AND** the named pipe is closed
- **AND** the process exits with code 0

#### Scenario: Client spawns service on Windows

- **WHEN** the Tauri client starts on Windows
- **AND** the service is not already running
- **THEN** the client spawns the service process
- **AND** the service binary is located in the same directory as the client
- **OR** the service binary is located in a trusted installation directory
- **AND** the client waits for the named pipe to become available
- **AND** the client connects to the service

