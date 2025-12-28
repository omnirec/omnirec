# service-architecture Specification Delta

## ADDED Requirements

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
- **THEN** the IPC socket is created at `$XDG_RUNTIME_DIR/omnirec.sock`
- **AND** the socket has appropriate permissions (owner only)

#### Scenario: Unix socket on macOS

- **WHEN** the service runs on macOS
- **THEN** the IPC socket is created at `$TMPDIR/omnirec.sock`
- **AND** the socket has appropriate permissions (owner only)

#### Scenario: Named pipe on Windows

- **WHEN** the service runs on Windows
- **THEN** a named pipe is created at `\\.\pipe\omnirec`
- **AND** the pipe has appropriate security descriptor

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
