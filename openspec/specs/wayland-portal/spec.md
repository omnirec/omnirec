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

### Requirement: Picker Fallback to Standard Picker

The system SHALL fall back to the standard Hyprland picker when OmniRec is not available to handle the screen capture request.

#### Scenario: Fallback when OmniRec not running

- **WHEN** the picker receives a screencast request from XDPH
- **AND** the IPC connection to OmniRec fails (app not running)
- **THEN** the picker executes `hyprland-share-picker` as a subprocess
- **AND** passes through all environment variables provided by XDPH
- **AND** forwards the standard picker's stdout output to XDPH
- **AND** exits with the standard picker's exit code

#### Scenario: Fallback when OmniRec has no selection

- **WHEN** the picker receives a screencast request from XDPH
- **AND** the IPC connection succeeds
- **AND** OmniRec responds with `NoSelection`
- **THEN** the picker executes `hyprland-share-picker` as a subprocess
- **AND** the user sees the standard picker UI to make a selection
- **AND** the selection is forwarded to XDPH

#### Scenario: OmniRec handles request when selection available and approved

- **WHEN** the picker receives a screencast request from XDPH
- **AND** the IPC connection to OmniRec succeeds
- **AND** OmniRec responds with a valid selection
- **AND** either a valid approval token exists OR the user approves via the dialog
- **THEN** the picker outputs the selection directly (no fallback)
- **AND** the standard picker is not invoked

#### Scenario: Standard picker not found

- **WHEN** the picker falls back to the standard picker
- **AND** `hyprland-share-picker` is not found in PATH
- **THEN** the picker logs an error message
- **AND** the picker exits with failure code
- **AND** XDPH cancels the portal request

### Requirement: Picker Approval Dialog

The picker SHALL display a graphical approval dialog when OmniRec has a pending recording request and no valid approval token exists.

#### Scenario: First-time approval dialog shown

- **WHEN** the picker receives a screencast request from XDPH
- **AND** the IPC connection to OmniRec succeeds
- **AND** OmniRec responds with a valid selection (pending recording request)
- **AND** no valid approval token exists
- **THEN** the picker displays a Qt6 dialog asking "Allow OmniRec to record your screen?"
- **AND** the dialog includes Allow and Deny buttons
- **AND** the dialog includes an "Always Allow" option

#### Scenario: User approves recording

- **GIVEN** the approval dialog is displayed
- **WHEN** the user clicks Allow Once or Always Allow
- **THEN** the picker outputs the selection to stdout in XDPH format
- **AND** the picker exits with success code

#### Scenario: User denies recording

- **GIVEN** the approval dialog is displayed
- **WHEN** the user clicks Deny
- **THEN** the picker exits with failure code
- **AND** no selection is output to stdout
- **AND** XDPH cancels the portal request

#### Scenario: User enables always allow

- **GIVEN** the approval dialog is displayed
- **WHEN** the user clicks "Always Allow"
- **THEN** the picker generates a random 256-bit approval token
- **AND** the picker sends the token to OmniRec via IPC StoreToken message
- **AND** OmniRec stores the token in the state directory
- **AND** the picker outputs the selection to stdout

#### Scenario: Dialog closed without selection

- **GIVEN** the approval dialog is displayed
- **WHEN** the user closes the dialog window without clicking a button
- **THEN** the picker exits with failure code
- **AND** XDPH cancels the portal request

### Requirement: Approval Token Validation

The picker SHALL validate approval tokens with OmniRec before displaying the approval dialog.

#### Scenario: Valid token bypasses dialog

- **WHEN** the picker receives a screencast request from XDPH
- **AND** OmniRec responds with a valid selection
- **AND** OmniRec indicates an approval token exists
- **AND** the picker sends ValidateToken to OmniRec
- **AND** OmniRec responds with TokenValid
- **THEN** the picker outputs the selection directly without showing a dialog
- **AND** the user experience matches the previous auto-approve behavior

#### Scenario: Invalid token shows dialog

- **WHEN** the picker receives a screencast request from XDPH
- **AND** OmniRec responds with a valid selection
- **AND** OmniRec indicates an approval token exists
- **AND** the picker sends ValidateToken to OmniRec
- **AND** OmniRec responds with TokenInvalid
- **THEN** the picker displays the approval dialog
- **AND** the user must explicitly approve or deny

#### Scenario: No token shows dialog

- **WHEN** the picker receives a screencast request from XDPH
- **AND** OmniRec responds with a valid selection
- **AND** OmniRec indicates no approval token exists
- **THEN** the picker displays the approval dialog
- **AND** the user must explicitly approve or deny

### Requirement: Approval Token Storage

The OmniRec application SHALL store and manage approval tokens in the XDG state directory.

#### Scenario: Token stored on first approval

- **WHEN** the picker sends a StoreToken IPC message with a token
- **THEN** OmniRec writes the token to `$XDG_STATE_HOME/omnirec/approval-token`
- **AND** the file permissions are set to 0600 (owner read/write only)
- **AND** OmniRec responds with TokenStored

#### Scenario: Token file location

- **GIVEN** XDG_STATE_HOME is set
- **WHEN** OmniRec stores or reads the approval token
- **THEN** the token file path is `$XDG_STATE_HOME/omnirec/approval-token`

#### Scenario: Token file location fallback

- **GIVEN** XDG_STATE_HOME is not set
- **WHEN** OmniRec stores or reads the approval token
- **THEN** the token file path is `~/.local/state/omnirec/approval-token`

#### Scenario: Token validation succeeds

- **WHEN** OmniRec receives a ValidateToken IPC message
- **AND** the provided token matches the stored token
- **THEN** OmniRec responds with TokenValid

#### Scenario: Token validation fails - mismatch

- **WHEN** OmniRec receives a ValidateToken IPC message
- **AND** the provided token does not match the stored token
- **THEN** OmniRec responds with TokenInvalid

#### Scenario: Token validation fails - no token stored

- **WHEN** OmniRec receives a ValidateToken IPC message
- **AND** no approval token file exists
- **THEN** OmniRec responds with TokenInvalid

### Requirement: IPC Protocol Extensions

The IPC protocol SHALL be extended to support approval token operations.

#### Scenario: Selection response includes token status

- **WHEN** the picker sends QuerySelection to OmniRec
- **AND** OmniRec has a valid selection
- **THEN** the response includes `has_approval_token: bool` indicating whether a stored token exists

#### Scenario: ValidateToken request

- **WHEN** the picker sends `{"type": "validate_token", "token": "<hex_string>"}`
- **THEN** OmniRec validates the token against the stored value
- **AND** responds with `{"type": "token_valid"}` or `{"type": "token_invalid"}`

#### Scenario: StoreToken request

- **WHEN** the picker sends `{"type": "store_token", "token": "<hex_string>"}`
- **THEN** OmniRec stores the token to the state directory
- **AND** responds with `{"type": "token_stored"}` or `{"type": "error", "message": "..."}`

