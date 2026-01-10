## ADDED Requirements

### Requirement: Show Transcript Window Setting

The configuration view SHALL provide a setting to control automatic transcript window display during recording.

#### Scenario: Setting displayed in configuration

- **WHEN** the configuration view is active
- **AND** transcription is enabled
- **THEN** a "Show transcript when recording starts" checkbox SHALL be visible
- **AND** the checkbox SHALL appear in the transcription settings area

#### Scenario: Setting default value

- **WHEN** the application starts with no saved configuration
- **THEN** the "Show transcript when recording starts" checkbox SHALL be checked by default

#### Scenario: Setting enabled

- **WHEN** the user checks the "Show transcript when recording starts" checkbox
- **THEN** the setting SHALL be saved to the configuration file automatically
- **AND** subsequent recordings with transcription SHALL automatically open the transcript window

#### Scenario: Setting disabled

- **WHEN** the user unchecks the "Show transcript when recording starts" checkbox
- **THEN** the setting SHALL be saved to the configuration file automatically
- **AND** subsequent recordings with transcription SHALL NOT automatically open the transcript window

#### Scenario: Setting visibility

- **WHEN** transcription is disabled
- **THEN** the "Show transcript when recording starts" checkbox SHALL be hidden
- **AND** the checkbox SHALL become visible when transcription is enabled

### Requirement: Transcript Window Setting Persistence

The application SHALL persist the transcript window setting alongside other transcription settings.

#### Scenario: Setting saved

- **WHEN** the user changes the "Show transcript when recording starts" setting
- **THEN** the setting SHALL be saved to the config file automatically
- **AND** the config file SHALL include a `transcription.show_window` boolean field

#### Scenario: Setting loaded on startup

- **WHEN** the application starts
- **THEN** the transcript window setting SHALL be loaded from the config file
- **AND** the checkbox SHALL reflect the saved value
