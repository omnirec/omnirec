## ADDED Requirements

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

#### Scenario: OmniRec handles request when selection available

- **WHEN** the picker receives a screencast request from XDPH
- **AND** the IPC connection to OmniRec succeeds
- **AND** OmniRec responds with a valid selection
- **THEN** the picker outputs the selection directly (no fallback)
- **AND** the standard picker is not invoked

#### Scenario: Standard picker not found

- **WHEN** the picker falls back to the standard picker
- **AND** `hyprland-share-picker` is not found in PATH
- **THEN** the picker logs an error message
- **AND** the picker exits with failure code
- **AND** XDPH cancels the portal request
