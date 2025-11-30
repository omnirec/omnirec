# recording-control Specification Delta

## MODIFIED Requirements

### Requirement: Recording State Management

The system SHALL maintain a recording state machine with states: Idle, Recording, and Saving.

#### Scenario: Initial state

- **WHEN** the application starts
- **THEN** the recording state is Idle
- **AND** the record button displays "Record"
- **AND** the capture mode selector defaults to "Window"
- **AND** the window list is enabled for selection

#### Scenario: Transition to Recording state

- **WHEN** the user clicks the record button while in Idle state
- **AND** a valid capture target is selected (window or region)
- **THEN** the state transitions to Recording
- **AND** the record button displays "Stop"
- **AND** the capture mode selector is disabled
- **AND** the window list or region selection UI is disabled

#### Scenario: Transition to Saving state

- **WHEN** the user clicks the stop button while in Recording state
- **THEN** the state transitions to Saving
- **AND** the record button is disabled
- **AND** a "Saving..." indicator is displayed

#### Scenario: Transition back to Idle state

- **WHEN** the recording file has been saved successfully
- **THEN** the state transitions to Idle
- **AND** the user is notified of the saved file location
- **AND** the capture mode selector is re-enabled
- **AND** the appropriate selection UI (window list or region) is re-enabled

### Requirement: Start Recording

The system SHALL begin capturing and encoding video when recording is started.

#### Scenario: Start recording successfully

- **WHEN** the user initiates recording
- **THEN** frame capture begins from the selected capture target
- **AND** frames are piped to the FFmpeg encoder process
- **AND** a recording duration timer is displayed

#### Scenario: Start recording with invalid window

- **WHEN** the user initiates recording in window mode
- **AND** the selected window is no longer valid
- **THEN** an error message is displayed
- **AND** the state remains Idle
- **AND** the window list is refreshed

#### Scenario: Start recording with invalid region

- **WHEN** the user initiates recording in region mode
- **AND** the selected region's monitor is no longer available
- **THEN** an error message is displayed
- **AND** the state remains Idle
- **AND** the user is prompted to select a new region
