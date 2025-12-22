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
