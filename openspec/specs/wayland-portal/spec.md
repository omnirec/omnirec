# wayland-portal Specification

## Purpose
TBD - created by archiving change add-wayland-linux-support. Update Purpose after archive.
## Requirements
### Requirement: Transparent Portal Backend

The system SHALL implement a custom xdg-desktop-portal backend that automatically approves capture requests based on the user's selection in the main application UI, without displaying any portal UI.

#### Scenario: Portal auto-approves user selection

- **WHEN** the user clicks Record in the application
- **AND** the portal backend receives a screencast request
- **THEN** the portal backend retrieves the user's capture selection from app state
- **AND** the portal backend responds with approval for that specific source
- **AND** no portal UI is displayed to the user

#### Scenario: Recording starts seamlessly

- **WHEN** the user has selected a capture target (window/display/region)
- **AND** the user clicks Record
- **THEN** the portal handshake completes transparently
- **AND** recording begins immediately
- **AND** the user experience is identical to Windows

#### Scenario: Portal backend registered as D-Bus service

- **WHEN** the application starts on Wayland
- **THEN** the portal backend registers on the session D-Bus
- **AND** it implements `org.freedesktop.impl.portal.ScreenCast` interface
- **AND** it is available to handle portal requests

### Requirement: Portal Session Management

The system SHALL manage portal sessions to enable efficient re-recording of the same target.

#### Scenario: Reuse existing portal session

- **WHEN** a portal session exists for the current capture target
- **AND** the user starts a new recording of the same target
- **THEN** the existing PipeWire stream is reused
- **AND** no new portal handshake is required

#### Scenario: New target requires new session

- **WHEN** the user selects a different capture target
- **AND** starts recording
- **THEN** a new portal session is created
- **AND** the portal handshake completes for the new target

#### Scenario: Session invalidated on target unavailable

- **WHEN** the captured window is closed or display is disconnected
- **THEN** the portal session is invalidated
- **AND** a new session is created on next recording attempt

### Requirement: Portal Client Integration

The system SHALL use a portal client to initiate screencast requests to xdg-desktop-portal.

#### Scenario: Create screencast session

- **WHEN** recording is initiated on Wayland
- **THEN** a D-Bus call is made to `org.freedesktop.portal.ScreenCast.CreateSession`
- **AND** the session handle is stored for subsequent operations

#### Scenario: Select capture source

- **WHEN** a session is created
- **THEN** a D-Bus call is made to `org.freedesktop.portal.ScreenCast.SelectSources`
- **AND** the source type (window/monitor) matches the user's UI selection
- **AND** cursor capture is enabled

#### Scenario: Start screencast stream

- **WHEN** sources have been selected
- **THEN** a D-Bus call is made to `org.freedesktop.portal.ScreenCast.Start`
- **AND** the portal backend auto-approves the request
- **AND** the response includes the PipeWire node ID

### Requirement: Portal Source Type Mapping

The system SHALL correctly map application capture modes to portal source types.

#### Scenario: Window capture source type

- **WHEN** the user has selected a window in the app UI
- **THEN** the portal `SelectSources` call specifies `types: WINDOW (2)`
- **AND** the portal backend approves capture of that specific window

#### Scenario: Display capture source type

- **WHEN** the user has selected a display in the app UI
- **THEN** the portal `SelectSources` call specifies `types: MONITOR (1)`
- **AND** the portal backend approves capture of that specific monitor

#### Scenario: Region capture source type

- **WHEN** the user has selected a region in the app UI
- **THEN** the portal `SelectSources` call specifies `types: MONITOR (1)`
- **AND** the portal backend approves capture of the region's monitor
- **AND** region cropping is handled by the application after stream setup

### Requirement: Shared State for Portal Coordination

The system SHALL maintain shared state between the main application and portal backend to communicate the user's selection.

#### Scenario: Selection stored before recording

- **WHEN** the user selects a capture target in the app UI
- **THEN** the selection is stored in shared state accessible to the portal backend
- **AND** the selection includes the target type and identifier

#### Scenario: Portal backend reads selection

- **WHEN** the portal backend receives a screencast request
- **THEN** it reads the current selection from shared state
- **AND** uses that selection to formulate the approval response

#### Scenario: Selection cleared after session end

- **WHEN** a recording session ends
- **THEN** the selection remains available for re-recording
- **AND** the selection is updated when user chooses a new target

### Requirement: Portal Error Handling

The system SHALL handle portal errors gracefully without disrupting the user experience.

#### Scenario: Portal not available

- **WHEN** xdg-desktop-portal is not running
- **THEN** an error is displayed: "Screen capture portal not available"
- **AND** capture functionality is disabled

#### Scenario: Portal backend registration failed

- **WHEN** the portal backend fails to register on D-Bus
- **THEN** an error is logged
- **AND** the user is informed that screen capture is unavailable

#### Scenario: PipeWire connection failed

- **WHEN** the portal returns a node ID but PipeWire connection fails
- **THEN** an error is displayed indicating capture failed
- **AND** the portal session is closed
- **AND** the user can retry by clicking Record again

### Requirement: Portal Restore Token Support

The system SHALL support portal restore tokens for persistent permissions across app restarts when available.

#### Scenario: Store restore token

- **WHEN** a portal session is successfully established
- **AND** the portal provides a restore token
- **THEN** the token is stored persistently
- **AND** the token is associated with the capture target identifier

#### Scenario: Use restore token on restart

- **WHEN** the application starts
- **AND** the user selects a previously-authorized target
- **AND** a restore token exists for that target
- **THEN** the portal request includes the restore token
- **AND** the session may be restored without full handshake

#### Scenario: Restore token expired or invalid

- **WHEN** a restore token is rejected by the portal
- **THEN** the normal portal flow is used
- **AND** the invalid token is discarded

