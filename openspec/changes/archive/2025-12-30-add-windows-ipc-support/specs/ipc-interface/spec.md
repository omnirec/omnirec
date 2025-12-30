## ADDED Requirements

### Requirement: Windows Named Pipe Client

The Tauri client SHALL connect to the service via named pipe on Windows.

#### Scenario: Connect to service via named pipe

- **WHEN** the client attempts to connect on Windows
- **THEN** the client opens the named pipe at `\\.\pipe\omnirec-service`
- **AND** the connection uses byte-mode for stream compatibility
- **AND** the existing length-prefixed JSON protocol works unchanged

#### Scenario: Named pipe connection timeout

- **WHEN** the client attempts to connect on Windows
- **AND** the service is not running
- **THEN** the client waits up to 10 seconds for the pipe to become available
- **AND** the client returns a timeout error if the pipe is not created

#### Scenario: Named pipe reconnection

- **WHEN** the client loses connection to the service on Windows
- **THEN** the client can attempt to reconnect
- **AND** if reconnection fails, the client can spawn a new service instance

### Requirement: Windows IPC Error Handling

The IPC client SHALL provide clear error messages for Windows-specific failures.

#### Scenario: Named pipe not found

- **WHEN** the client attempts to connect on Windows
- **AND** the named pipe does not exist
- **THEN** the client returns a ConnectionFailed error
- **AND** the error message indicates the service may not be running

#### Scenario: Named pipe access denied

- **WHEN** the client attempts to connect on Windows
- **AND** the security descriptor denies access
- **THEN** the client returns a ConnectionFailed error
- **AND** the error message indicates a permissions issue

#### Scenario: Service binary not found on Windows

- **WHEN** the client attempts to spawn the service on Windows
- **AND** the service binary cannot be found
- **THEN** the client returns a ConnectionFailed error
- **AND** the error message indicates the service binary location
