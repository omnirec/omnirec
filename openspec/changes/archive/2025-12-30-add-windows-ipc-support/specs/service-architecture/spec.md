## MODIFIED Requirements

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

## ADDED Requirements

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
