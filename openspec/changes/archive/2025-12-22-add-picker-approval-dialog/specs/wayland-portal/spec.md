## ADDED Requirements

### Requirement: Picker Approval Dialog

The picker SHALL display a graphical approval dialog when OmniRec has a pending recording request and no valid approval token exists.

#### Scenario: First-time approval dialog shown

- **WHEN** the picker receives a screencast request from XDPH
- **AND** the IPC connection to OmniRec succeeds
- **AND** OmniRec responds with a valid selection (pending recording request)
- **AND** no valid approval token exists
- **THEN** the picker displays a GTK dialog asking "Allow OmniRec to record the screen?"
- **AND** the dialog includes Allow and Deny buttons
- **AND** the dialog includes a checkbox "Always allow OmniRec to record the screen"

#### Scenario: User approves recording

- **GIVEN** the approval dialog is displayed
- **WHEN** the user clicks Allow
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
- **WHEN** the user checks "Always allow OmniRec to record the screen"
- **AND** the user clicks Allow
- **THEN** the picker generates a random 256-bit approval token
- **AND** the picker sends the token to OmniRec via IPC StoreToken message
- **AND** OmniRec stores the token in the state directory
- **AND** the picker outputs the selection to stdout

#### Scenario: Dialog closed without selection

- **GIVEN** the approval dialog is displayed
- **WHEN** the user closes the dialog window without clicking Allow or Deny
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

## MODIFIED Requirements

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
