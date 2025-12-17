# app-configuration Specification

## Purpose
TBD - created by archiving change add-configuration-support. Update Purpose after archive.
## Requirements
### Requirement: Configuration Tab Access

The application SHALL provide a configuration tab accessible from the main capture mode tab bar.

#### Scenario: Config button displayed in tab bar

- **WHEN** the application window is displayed
- **THEN** a gear icon button SHALL be visible on the right side of the capture mode tab bar
- **AND** the button SHALL be visually distinct as a settings/config action

#### Scenario: Config tab activation

- **WHEN** the user clicks the gear icon button
- **THEN** the configuration view SHALL be displayed
- **AND** the capture mode tabs (Window, Region, Display) SHALL appear inactive
- **AND** the gear button SHALL appear active/selected

#### Scenario: Return to capture mode

- **WHEN** the user clicks any capture mode tab (Window, Region, Display)
- **THEN** the corresponding capture view SHALL be displayed
- **AND** the gear button SHALL appear inactive

#### Scenario: Config tab disabled during recording

- **WHEN** a recording is in progress
- **THEN** the gear icon button SHALL be disabled
- **AND** the user SHALL NOT be able to switch to the configuration view

### Requirement: Configuration View Layout

The configuration view SHALL display settings organized into logical groups.

#### Scenario: Output group displayed

- **WHEN** the configuration view is active
- **THEN** an "Output" group section SHALL be visible
- **AND** the group SHALL contain the output directory setting

#### Scenario: Group visual structure

- **WHEN** a settings group is displayed
- **THEN** the group SHALL have a header label identifying the group name
- **AND** settings within the group SHALL be visually contained together

### Requirement: Output Directory Setting

The configuration view SHALL allow users to customize the recording output directory.

#### Scenario: Default directory displayed as placeholder

- **WHEN** no custom output directory has been configured
- **THEN** the output directory input field SHALL display the system default path as grayed placeholder text
- **AND** the input field value SHALL be empty (not set)

#### Scenario: Custom directory entry via text input

- **WHEN** the user types a path into the output directory input field
- **AND** the input loses focus OR typing stops for 500ms
- **THEN** the entered path SHALL be validated
- **AND** if valid, the path SHALL be saved to the configuration file automatically

#### Scenario: Custom directory entry via folder picker

- **WHEN** the user clicks the folder picker button next to the output directory input
- **THEN** a native folder selection dialog SHALL open
- **AND** when a folder is selected, the path SHALL populate the input field
- **AND** the path SHALL be saved to the configuration file automatically

#### Scenario: Invalid directory handling

- **WHEN** the user enters a directory path that does not exist or is not writable
- **THEN** an inline error message SHALL be displayed below the input
- **AND** the invalid path SHALL NOT be saved to the configuration
- **AND** the previous valid value SHALL be retained

#### Scenario: Clear custom directory

- **WHEN** the user clears the output directory input field (empties it)
- **AND** the input loses focus
- **THEN** the configuration SHALL revert to using the system default
- **AND** the placeholder SHALL again show the default path
- **AND** the change SHALL be saved automatically

### Requirement: Configuration Persistence

The application SHALL persist configuration to a file in the platform-standard configuration directory.

#### Scenario: Config file location on Linux

- **WHEN** the application saves configuration on Linux
- **THEN** the config file SHALL be written to `~/.config/omnirec/config.json`

#### Scenario: Config file location on macOS

- **WHEN** the application saves configuration on macOS
- **THEN** the config file SHALL be written to `~/Library/Application Support/omnirec/config.json`

#### Scenario: Config file location on Windows

- **WHEN** the application saves configuration on Windows
- **THEN** the config file SHALL be written to `%APPDATA%\omnirec\config.json`

#### Scenario: Config loaded on startup

- **WHEN** the application starts
- **THEN** the configuration SHALL be loaded from the config file if it exists
- **AND** the UI SHALL reflect the loaded configuration values

#### Scenario: Config file does not exist

- **WHEN** the application starts and no config file exists
- **THEN** the application SHALL use default values for all settings
- **AND** no error SHALL be displayed

#### Scenario: Config file corrupted

- **WHEN** the application attempts to load a corrupted or invalid config file
- **THEN** the application SHALL fall back to default values
- **AND** the application SHALL log a warning (not display error to user)

### Requirement: Recording Uses Configured Output Directory

The recording system SHALL use the configured output directory when saving recordings.

#### Scenario: Recording with custom output directory

- **WHEN** a recording completes
- **AND** a custom output directory is configured
- **THEN** the recording file SHALL be saved to the custom directory

#### Scenario: Recording with default output directory

- **WHEN** a recording completes
- **AND** no custom output directory is configured
- **THEN** the recording file SHALL be saved to the system default Videos folder

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

