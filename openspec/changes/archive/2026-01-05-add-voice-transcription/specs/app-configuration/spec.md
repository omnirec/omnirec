# app-configuration Specification Delta

## ADDED Requirements

### Requirement: Transcription Setting

The configuration view SHALL provide a toggle to enable or disable voice transcription.

#### Scenario: Transcription toggle in settings

- **WHEN** the configuration view is active
- **THEN** a "Transcribe voice" checkbox SHALL be visible in the Audio group
- **AND** the checkbox SHALL appear after the Echo Cancellation toggle

#### Scenario: Transcription toggle default state

- **WHEN** the application starts with no saved configuration
- **THEN** the transcription toggle SHALL be disabled by default

#### Scenario: Transcription toggle enable

- **WHEN** the user enables the transcription toggle
- **THEN** voice transcription SHALL be enabled for future recordings
- **AND** the setting SHALL be saved to the configuration file automatically

#### Scenario: Transcription toggle disable

- **WHEN** the user disables the transcription toggle
- **THEN** voice transcription SHALL be disabled for future recordings
- **AND** the setting SHALL be saved to the configuration file automatically

#### Scenario: Transcription toggle hint

- **WHEN** the transcription toggle is displayed
- **THEN** a hint SHALL appear below stating "Transcribe speech to a text file (requires whisper model)"

### Requirement: Transcription Quick Toggle

The main UI SHALL provide a quick toggle for transcription on the record controls row.

#### Scenario: Quick toggle visibility when system audio enabled

- **WHEN** a system audio source is selected (Linux/Windows dropdown or macOS checkbox)
- **THEN** a "Transcribe voice" checkbox SHALL be visible in the controls section
- **AND** the checkbox SHALL be right-aligned on the same row as the Record button

#### Scenario: Quick toggle visibility when system audio disabled

- **WHEN** no system audio source is selected
- **THEN** the "Transcribe voice" checkbox SHALL be hidden in the controls section

#### Scenario: Quick toggle synchronization

- **WHEN** the user changes the quick toggle in the main UI
- **THEN** the settings toggle SHALL update to match
- **AND** the setting SHALL be saved to the configuration file automatically

#### Scenario: Settings toggle synchronization

- **WHEN** the user changes the transcription toggle in settings
- **THEN** the quick toggle in the main UI SHALL update to match

#### Scenario: Quick toggle disabled during recording

- **WHEN** a recording is in progress
- **THEN** the quick toggle SHALL be disabled
- **AND** the toggle state SHALL NOT be changeable until recording stops

### Requirement: Transcription Configuration Persistence

The application SHALL persist transcription configuration alongside other settings.

#### Scenario: Transcription config saved

- **WHEN** the user changes the transcription setting
- **THEN** the setting SHALL be saved to the config file automatically
- **AND** the config file SHALL include a `transcription.enabled` boolean field

#### Scenario: Transcription config loaded on startup

- **WHEN** the application starts
- **THEN** the transcription configuration SHALL be loaded from the config file
- **AND** both the settings toggle and quick toggle SHALL reflect the loaded value
