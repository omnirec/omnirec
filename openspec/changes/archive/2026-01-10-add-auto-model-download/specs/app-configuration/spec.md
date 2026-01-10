## ADDED Requirements

### Requirement: Transcription Model Selection

The configuration view SHALL provide a dropdown to select the whisper model for transcription.

#### Scenario: Model dropdown in settings

- **WHEN** the configuration view is active
- **THEN** a "Transcription model" dropdown SHALL be visible in the Audio group
- **AND** the dropdown SHALL appear after the "Transcribe voice" checkbox

#### Scenario: Model dropdown options

- **WHEN** the user opens the model dropdown
- **THEN** available whisper models SHALL be listed
- **AND** each option SHALL display the model name and size (e.g., "medium.en (1.5 GB)")
- **AND** English-only models SHALL be grouped before multilingual variants

#### Scenario: Model selection default

- **WHEN** the application starts with no saved configuration
- **THEN** the model dropdown SHALL display "medium.en" as the default selection

#### Scenario: Model selection persistence

- **WHEN** the user selects a different model
- **THEN** the selection SHALL be saved to the configuration file automatically
- **AND** the setting SHALL persist across application restarts

#### Scenario: Model dropdown visibility

- **WHEN** the "Transcribe voice" checkbox is disabled
- **THEN** the model dropdown SHALL be hidden or disabled
- **AND** the dropdown SHALL become visible/enabled when transcription is enabled

### Requirement: Model Download Controls

The configuration view SHALL provide controls for downloading the selected whisper model.

#### Scenario: Download button when model not present

- **WHEN** the selected model file does not exist on disk
- **THEN** a "Download" button SHALL be displayed next to the model dropdown
- **AND** the button SHALL indicate the download size

#### Scenario: Download button hidden when model present

- **WHEN** the selected model file exists on disk
- **THEN** the "Download" button SHALL be hidden
- **AND** a checkmark or "Downloaded" indicator SHALL be shown instead

#### Scenario: Download initiation

- **WHEN** the user clicks the "Download" button
- **THEN** the download SHALL begin immediately
- **AND** the button SHALL be replaced with a progress indicator

#### Scenario: Download progress display

- **WHEN** a model download is in progress
- **THEN** a progress bar SHALL be displayed
- **AND** the progress bar SHALL show percentage complete
- **AND** the downloaded/total size SHALL be displayed (e.g., "512 MB / 1.5 GB")

#### Scenario: Download cancel button

- **WHEN** a model download is in progress
- **THEN** a "Cancel" button SHALL be available
- **AND** clicking cancel SHALL abort the download
- **AND** the UI SHALL return to showing the "Download" button

#### Scenario: Model dropdown during download

- **WHEN** a model download is in progress
- **THEN** the model dropdown SHALL be disabled
- **AND** the user SHALL NOT be able to change model selection until download completes or is cancelled

#### Scenario: Download completion feedback

- **WHEN** a model download completes successfully
- **THEN** the progress bar SHALL be replaced with a success indicator
- **AND** the "Downloaded" status SHALL be shown
- **AND** the model dropdown SHALL be re-enabled

#### Scenario: Download error feedback

- **WHEN** a model download fails
- **THEN** an error message SHALL be displayed
- **AND** the error message SHALL include the failure reason
- **AND** the "Download" button SHALL reappear to allow retry

### Requirement: Model Configuration Persistence

The application SHALL persist model configuration alongside other transcription settings.

#### Scenario: Model config saved

- **WHEN** the user changes the model selection
- **THEN** the setting SHALL be saved to the config file automatically
- **AND** the config file SHALL include a `transcription.model` string field

#### Scenario: Model config loaded on startup

- **WHEN** the application starts
- **THEN** the model configuration SHALL be loaded from the config file
- **AND** the model dropdown SHALL reflect the saved selection
- **AND** the model status (downloaded/not downloaded) SHALL be checked and displayed
