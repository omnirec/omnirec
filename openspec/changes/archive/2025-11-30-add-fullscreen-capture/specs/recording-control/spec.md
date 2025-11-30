## ADDED Requirements

### Requirement: Start Display Recording

The system SHALL begin capturing an entire display when recording is started in display mode.

#### Scenario: Start display recording successfully

- **WHEN** the user initiates recording in display mode
- **AND** a display is selected
- **THEN** frame capture begins from the entire selected display
- **AND** frames are piped to the FFmpeg encoder process
- **AND** a recording duration timer is displayed

#### Scenario: Start recording with invalid display

- **WHEN** the user initiates recording in display mode
- **AND** the selected display is no longer available
- **THEN** an error message is displayed
- **AND** the state remains Idle
- **AND** the display list is refreshed
