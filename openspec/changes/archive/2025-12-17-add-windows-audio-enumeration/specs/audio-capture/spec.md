## ADDED Requirements

### Requirement: Windows Audio Source Enumeration

The system SHALL enumerate available audio devices on Windows using WASAPI.

#### Scenario: List audio sources on Windows

- **WHEN** the application starts on Windows
- **THEN** the system SHALL query available audio output devices (playback endpoints) via WASAPI
- **AND** the system SHALL query available audio input devices (capture endpoints) via WASAPI
- **AND** each device SHALL have a unique identifier (WASAPI endpoint ID), display name, and source type

#### Scenario: Windows output devices as system audio sources

- **WHEN** audio output devices are enumerated on Windows
- **THEN** each device SHALL be returned with `AudioSourceType::Output`
- **AND** the device name SHALL be the friendly name from WASAPI
- **AND** these devices can be used for system audio loopback capture

#### Scenario: Windows input devices as microphone sources

- **WHEN** audio input devices are enumerated on Windows
- **THEN** each device SHALL be returned with `AudioSourceType::Input`
- **AND** the device name SHALL be the friendly name from WASAPI
- **AND** these devices represent microphones and other recording inputs

#### Scenario: No audio devices available on Windows

- **WHEN** no audio devices are detected on Windows
- **THEN** the audio source list SHALL be empty
- **AND** audio recording SHALL be unavailable

#### Scenario: COM initialization failure on Windows

- **WHEN** COM initialization fails during enumeration
- **THEN** the system SHALL return an empty audio source list
- **AND** the system SHALL log a warning message
- **AND** the application SHALL NOT crash

## REMOVED Requirements

### Requirement: Windows Audio Capture Stub

**Reason**: Windows audio enumeration is now implemented via WASAPI. The stub requirement is superseded by the new Windows Audio Source Enumeration requirement.

**Migration**: The `list_audio_sources()` function now returns actual device data instead of `NotImplemented` error. Capture operations (`start_audio_capture`) remain stubbed until the `add-windows-audio-capture` change is implemented.
