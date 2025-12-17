# app-configuration Specification Delta

## ADDED Requirements

### Requirement: Audio Settings Group

The configuration view SHALL include an Audio settings group for controlling audio recording options.

#### Scenario: Audio group displayed

- **WHEN** the configuration view is active
- **THEN** an "Audio" group section SHALL be visible
- **AND** the group SHALL contain audio enable toggle and source selection

#### Scenario: Audio group position

- **WHEN** the configuration view is displayed
- **THEN** the Audio group SHALL appear after the Output group

### Requirement: Audio Recording Toggle

The configuration view SHALL provide a toggle to enable or disable audio recording.

#### Scenario: Audio toggle default state

- **WHEN** the application starts with no saved configuration
- **THEN** the audio recording toggle SHALL be enabled by default

#### Scenario: Audio toggle enable

- **WHEN** the user enables the audio recording toggle
- **THEN** audio recording SHALL be enabled for future recordings
- **AND** the setting SHALL be saved to the configuration file automatically

#### Scenario: Audio toggle disable

- **WHEN** the user disables the audio recording toggle
- **THEN** audio recording SHALL be disabled for future recordings
- **AND** the audio source dropdown SHALL be disabled (grayed out)
- **AND** the setting SHALL be saved to the configuration file automatically

### Requirement: Audio Source Selection

The configuration view SHALL provide a dropdown to select the audio source for recording.

#### Scenario: Audio source dropdown default state

- **WHEN** the application starts with no saved configuration
- **THEN** the audio source dropdown SHALL display "None" as the default selection
- **AND** no audio source SHALL be selected

#### Scenario: Audio source list populated

- **WHEN** the user opens the audio source dropdown
- **THEN** available audio output devices SHALL be listed
- **AND** each device SHALL display its name
- **AND** "None" SHALL be available as an option to disable audio

#### Scenario: Audio source selection

- **WHEN** the user selects an audio source from the dropdown
- **THEN** the selection SHALL be retained for subsequent recordings
- **AND** the dropdown SHALL display the selected source name
- **AND** the setting SHALL be saved to the configuration file automatically

#### Scenario: Audio source selection persistence

- **WHEN** the application restarts
- **THEN** the previously selected audio source SHALL be restored
- **AND** if the source is no longer available, the dropdown SHALL show "None"

#### Scenario: Audio source dropdown disabled

- **WHEN** audio recording is disabled via the toggle
- **THEN** the audio source dropdown SHALL be disabled
- **AND** the dropdown SHALL retain its last selection (for when audio is re-enabled)

### Requirement: Audio Configuration Persistence

The application SHALL persist audio configuration alongside other settings.

#### Scenario: Audio config saved

- **WHEN** the user changes audio settings
- **THEN** the settings SHALL be saved to the config file automatically
- **AND** the config file SHALL include audio enabled state and selected source ID

#### Scenario: Audio config loaded on startup

- **WHEN** the application starts
- **THEN** the audio configuration SHALL be loaded from the config file
- **AND** the UI SHALL reflect the loaded audio settings
