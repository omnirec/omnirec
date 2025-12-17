## ADDED Requirements

### Requirement: Microphone Source Selection

The configuration view SHALL provide a dropdown to select a microphone for recording.

#### Scenario: Microphone dropdown default state

- **WHEN** the application starts with no saved configuration
- **THEN** the microphone dropdown SHALL display "None" as the default selection
- **AND** no microphone SHALL be selected

#### Scenario: Microphone source list populated

- **WHEN** the user opens the microphone dropdown
- **THEN** available audio input devices (microphones) SHALL be listed
- **AND** each device SHALL display its name
- **AND** "None" SHALL be available as an option to disable microphone

#### Scenario: Microphone source selection

- **WHEN** the user selects a microphone from the dropdown
- **THEN** the selection SHALL be retained for subsequent recordings
- **AND** the dropdown SHALL display the selected microphone name
- **AND** the setting SHALL be saved to the configuration file automatically

#### Scenario: Microphone source selection persistence

- **WHEN** the application restarts
- **THEN** the previously selected microphone SHALL be restored
- **AND** if the microphone is no longer available, the dropdown SHALL show "None"

#### Scenario: Microphone dropdown disabled when audio disabled

- **WHEN** audio recording is disabled via the toggle
- **THEN** the microphone dropdown SHALL be disabled
- **AND** the dropdown SHALL retain its last selection (for when audio is re-enabled)

### Requirement: Echo Cancellation Toggle

The configuration view SHALL provide a toggle to enable or disable acoustic echo cancellation for microphone input.

#### Scenario: Echo cancellation toggle default state

- **WHEN** the application starts with no saved configuration
- **THEN** the echo cancellation toggle SHALL be enabled by default

#### Scenario: Echo cancellation toggle visibility

- **WHEN** a microphone is selected (not "None")
- **THEN** the echo cancellation toggle SHALL be visible
- **AND** the toggle SHALL be enabled/disabled based on saved configuration

#### Scenario: Echo cancellation toggle hidden when no microphone

- **WHEN** no microphone is selected (microphone is "None")
- **THEN** the echo cancellation toggle SHALL be hidden

#### Scenario: Echo cancellation enable

- **WHEN** the user enables the echo cancellation toggle
- **THEN** echo cancellation SHALL be enabled for future recordings
- **AND** the setting SHALL be saved to the configuration file automatically

#### Scenario: Echo cancellation disable

- **WHEN** the user disables the echo cancellation toggle
- **THEN** echo cancellation SHALL be disabled for future recordings
- **AND** the setting SHALL be saved to the configuration file automatically

### Requirement: Microphone Configuration Persistence

The application SHALL persist microphone configuration alongside other audio settings.

#### Scenario: Microphone config saved

- **WHEN** the user changes microphone settings
- **THEN** the settings SHALL be saved to the config file automatically
- **AND** the config file SHALL include microphone source ID and echo cancellation state

#### Scenario: Microphone config loaded on startup

- **WHEN** the application starts
- **THEN** the microphone configuration SHALL be loaded from the config file
- **AND** the UI SHALL reflect the loaded microphone settings

## MODIFIED Requirements

### Requirement: Audio Settings Group

The configuration view SHALL include an Audio settings group for controlling audio recording options.

#### Scenario: Audio group displayed

- **WHEN** the configuration view is active
- **THEN** an "Audio" group section SHALL be visible
- **AND** the group SHALL contain audio enable toggle, system audio source selection, microphone selection, and echo cancellation toggle

#### Scenario: Audio group position

- **WHEN** the configuration view is displayed
- **THEN** the Audio group SHALL appear after the Output group

### Requirement: Audio Configuration Persistence

The application SHALL persist audio configuration alongside other settings.

#### Scenario: Audio config saved

- **WHEN** the user changes audio settings
- **THEN** the settings SHALL be saved to the config file automatically
- **AND** the config file SHALL include audio enabled state, selected system audio source ID, selected microphone ID, and echo cancellation state

#### Scenario: Audio config loaded on startup

- **WHEN** the application starts
- **THEN** the audio configuration SHALL be loaded from the config file
- **AND** the UI SHALL reflect the loaded audio settings
