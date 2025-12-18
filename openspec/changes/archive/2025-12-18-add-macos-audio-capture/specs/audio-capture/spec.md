## MODIFIED Requirements

### Requirement: Audio Source Enumeration

The system SHALL enumerate available audio output devices for capture.

#### Scenario: List audio sources on Linux

- **WHEN** the application starts on Linux
- **THEN** the system SHALL query available audio output devices via PipeWire
- **AND** the system SHALL query available audio input devices (microphones) via PipeWire
- **AND** each device SHALL have a unique identifier, display name, and source type

#### Scenario: List audio sources on macOS

- **WHEN** the application starts on macOS
- **THEN** the system SHALL query available audio output devices via Core Audio
- **AND** the system SHALL query available audio input devices (microphones) via Core Audio
- **AND** each device SHALL have a unique identifier (AudioDeviceID), display name, and source type

#### Scenario: Refresh audio sources

- **WHEN** the user opens the audio source dropdown
- **THEN** the system SHALL refresh the list of available audio devices
- **AND** previously disconnected devices SHALL be removed
- **AND** newly connected devices SHALL appear

#### Scenario: No audio sources available

- **WHEN** no audio output devices are detected
- **THEN** the audio source dropdown SHALL display "No audio sources found"
- **AND** audio recording SHALL be unavailable

## REMOVED Requirements

### Requirement: macOS Audio Capture Stub

**Reason**: macOS audio capture is now implemented using ScreenCaptureKit.

**Migration**: The stub behavior is replaced by actual implementation. Code that catches `NotImplemented` errors for macOS audio should be updated to handle successful capture.

## ADDED Requirements

### Requirement: macOS Audio Source Enumeration

The system SHALL enumerate available audio devices on macOS using Core Audio.

#### Scenario: List audio sources on macOS

- **WHEN** the application starts on macOS
- **THEN** the system SHALL query available audio output devices (playback endpoints) via Core Audio
- **AND** the system SHALL query available audio input devices (capture endpoints) via Core Audio
- **AND** each device SHALL have a unique identifier (AudioDeviceID as string), display name, and source type

#### Scenario: macOS output devices as system audio sources

- **WHEN** audio output devices are enumerated on macOS
- **THEN** each device SHALL be returned with `AudioSourceType::Output`
- **AND** the device name SHALL be the device's friendly name from Core Audio
- **AND** these devices represent potential system audio capture sources

#### Scenario: macOS input devices as microphone sources

- **WHEN** audio input devices are enumerated on macOS
- **THEN** each device SHALL be returned with `AudioSourceType::Input`
- **AND** the device name SHALL be the device's friendly name from Core Audio
- **AND** these devices represent microphones and other recording inputs

#### Scenario: No audio devices available on macOS

- **WHEN** no audio devices are detected on macOS
- **THEN** the audio source list SHALL be empty
- **AND** audio recording SHALL be unavailable

#### Scenario: Core Audio enumeration failure on macOS

- **WHEN** Core Audio initialization or enumeration fails
- **THEN** the system SHALL return an empty audio source list
- **AND** the system SHALL log a warning message
- **AND** the application SHALL NOT crash

### Requirement: macOS System Audio Capture

The system SHALL capture system audio on macOS using ScreenCaptureKit.

#### Scenario: ScreenCaptureKit audio capture

- **WHEN** audio capture starts on macOS 13+
- **THEN** the system SHALL create an SCStream with `capturesAudio` enabled
- **AND** the stream SHALL capture all system audio output (not per-device)
- **AND** audio samples SHALL be delivered through the frame delegate

#### Scenario: Audio sample format on macOS

- **WHEN** audio is being captured on macOS
- **THEN** samples SHALL be resampled to 48kHz sample rate in the capture module
- **AND** samples SHALL be stereo (2 channels)
- **AND** samples SHALL be 32-bit floating point (-1.0 to 1.0 range)

#### Scenario: Audio capture stop on macOS

- **WHEN** the stop handle is set during capture on macOS
- **THEN** the SCStream SHALL stop capturing
- **AND** remaining buffered samples SHALL be flushed to the channel
- **AND** SCStream resources SHALL be released

#### Scenario: macOS version check for audio capture

- **WHEN** audio capture is requested on macOS versions prior to 13
- **THEN** the system SHALL return a `CaptureError::NotImplemented` error
- **AND** the error message SHALL state "System audio capture requires macOS 13 or later"

#### Scenario: Screen recording permission required

- **WHEN** audio capture is requested on macOS
- **AND** screen recording permission has not been granted
- **THEN** the system SHALL return a `CaptureError::PermissionDenied` error
- **AND** the error message SHALL guide the user to System Settings > Privacy & Security > Screen Recording

#### Scenario: Permission prompt triggered

- **WHEN** audio capture is requested on macOS
- **AND** screen recording permission status is undetermined
- **THEN** the system SHALL trigger the macOS permission prompt
- **AND** the system SHALL return a `CaptureError::PermissionDenied` error with a message explaining that permission was requested
- **AND** the user SHALL be instructed to grant permission and retry

### Requirement: macOS Audio Sample Format

The system SHALL produce audio samples in a consistent format on macOS.

#### Scenario: macOS audio output format

- **WHEN** audio is being captured on macOS
- **THEN** samples SHALL be 48kHz sample rate
- **AND** samples SHALL be stereo (2 channels)
- **AND** samples SHALL be 32-bit floating point (-1.0 to 1.0 range)
- **AND** samples SHALL be interleaved (L, R, L, R, ...)

### Requirement: macOS System Audio UI

The system SHALL present a simplified UI for system audio on macOS since per-device selection is not supported.

#### Scenario: System audio checkbox on macOS

- **WHEN** the application is running on macOS
- **THEN** the system audio device dropdown SHALL be replaced with a checkbox labeled "Record system audio"
- **AND** the checkbox SHALL be unchecked by default

#### Scenario: Enable system audio recording on macOS

- **WHEN** the user checks the "Record system audio" checkbox on macOS
- **THEN** the recording SHALL include all system audio output
- **AND** the backend SHALL receive a boolean flag rather than a device ID

#### Scenario: System audio unavailable on macOS 12

- **WHEN** the application is running on macOS versions prior to 13
- **THEN** the "Record system audio" checkbox SHALL be disabled
- **AND** a tooltip or label SHALL explain that macOS 13+ is required for system audio
