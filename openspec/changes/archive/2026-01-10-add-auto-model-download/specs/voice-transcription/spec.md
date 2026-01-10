## MODIFIED Requirements

### Requirement: Whisper Model Management

The system SHALL support configurable whisper model selection with in-app download capability.

#### Scenario: Available models

- **WHEN** the user views model selection options
- **THEN** the system SHALL offer the following models:
- **AND** tiny.en (75 MB) - English optimized, fastest
- **AND** tiny (75 MB) - Multilingual
- **AND** base.en (142 MB) - English optimized
- **AND** base (142 MB) - Multilingual
- **AND** small.en (466 MB) - English optimized
- **AND** small (466 MB) - Multilingual
- **AND** medium.en (1.5 GB) - English optimized, default
- **AND** medium (1.5 GB) - Multilingual
- **AND** large-v3 (2.9 GB) - Multilingual, highest accuracy

#### Scenario: Model location

- **WHEN** transcription is enabled
- **THEN** the system SHALL look for the configured model at the platform-specific cache path
- **AND** on Linux the path SHALL be `~/.cache/omnirec/whisper/ggml-{model}.bin`
- **AND** on macOS the path SHALL be `~/Library/Caches/omnirec/whisper/ggml-{model}.bin`
- **AND** on Windows the path SHALL be `%LOCALAPPDATA%\omnirec\whisper\ggml-{model}.bin`

#### Scenario: Model not found at recording start

- **WHEN** recording starts with transcription enabled
- **AND** the configured whisper model file does not exist
- **THEN** the system SHALL prevent recording from starting
- **AND** the system SHALL display an error message stating the model must be downloaded
- **AND** the error message SHALL direct the user to the transcription settings

#### Scenario: Model loading

- **WHEN** recording starts with transcription enabled
- **AND** the model file exists
- **THEN** the system SHALL load the whisper model into memory
- **AND** the model SHALL remain loaded until the service stops or transcription is disabled

## ADDED Requirements

### Requirement: Model Download

The system SHALL provide in-app download capability for whisper models.

#### Scenario: Download source

- **WHEN** the user initiates a model download
- **THEN** the system SHALL download from Hugging Face repository
- **AND** the URL pattern SHALL be `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{model}.bin`

#### Scenario: Download progress

- **WHEN** a model download is in progress
- **THEN** the system SHALL emit progress events
- **AND** events SHALL include bytes downloaded, total bytes, and percentage
- **AND** events SHALL be emitted at least every 1% progress or every second

#### Scenario: Download completion

- **WHEN** a model download completes successfully
- **THEN** the system SHALL save the file to the platform-specific cache path
- **AND** the system SHALL emit a completion event

#### Scenario: Download failure

- **WHEN** a model download fails due to network error
- **THEN** the system SHALL emit an error event with the failure reason
- **AND** any partial download file SHALL be removed

#### Scenario: Download cancellation

- **WHEN** the user cancels a download in progress
- **THEN** the system SHALL abort the HTTP request
- **AND** any partial download file SHALL be removed
- **AND** the system SHALL emit a cancellation event

### Requirement: Model Status Query

The system SHALL provide model availability information.

#### Scenario: Model status response

- **WHEN** the application queries model status for a specific model
- **THEN** the response SHALL include the model name
- **AND** the response SHALL include the expected file path
- **AND** the response SHALL include whether the file exists
- **AND** the response SHALL include the file size if it exists

#### Scenario: List available models

- **WHEN** the application requests the list of available models
- **THEN** the response SHALL include all supported model identifiers
- **AND** each model entry SHALL include the display name
- **AND** each model entry SHALL include the download size
- **AND** each model entry SHALL include a brief description
