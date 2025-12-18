## ADDED Requirements

### Requirement: Windows Audio Capture

The system SHALL capture audio from Windows audio devices using WASAPI.

#### Scenario: WASAPI loopback capture for system audio

- **WHEN** audio capture starts on Windows with an output device selected
- **THEN** the system SHALL use WASAPI loopback mode to capture system audio
- **AND** the captured audio SHALL include all audio played through the selected output device
- **AND** the audio SHALL be delivered as 48kHz stereo f32 samples

#### Scenario: WASAPI direct capture for microphone

- **WHEN** audio capture starts on Windows with an input device selected
- **THEN** the system SHALL use WASAPI capture mode on the input endpoint
- **AND** the captured audio SHALL include microphone input from the selected device
- **AND** the audio SHALL be delivered as 48kHz stereo f32 samples

#### Scenario: Format conversion from device native format

- **WHEN** the audio device provides samples in a format other than 48kHz stereo float32
- **THEN** the system SHALL convert samples to 48kHz stereo float32
- **AND** mono audio SHALL be converted to stereo by duplicating the channel
- **AND** 16-bit integer samples SHALL be converted to float32

#### Scenario: Audio capture stop

- **WHEN** the stop handle is set during capture
- **THEN** the WASAPI capture thread SHALL stop capturing
- **AND** remaining buffered samples SHALL be flushed to the channel
- **AND** WASAPI resources SHALL be released

#### Scenario: Device disconnection during capture

- **WHEN** the audio device disconnects during capture
- **THEN** the capture thread SHALL stop gracefully
- **AND** the audio sample channel SHALL be closed
- **AND** video recording SHALL continue without audio

### Requirement: Windows Audio Sample Format

The system SHALL produce audio samples in a consistent format on Windows.

#### Scenario: Windows audio output format

- **WHEN** audio is being captured on Windows
- **THEN** samples SHALL be 48kHz sample rate
- **AND** samples SHALL be stereo (2 channels)
- **AND** samples SHALL be 32-bit floating point (-1.0 to 1.0 range)
- **AND** samples SHALL be interleaved (L, R, L, R, ...)

## MODIFIED Requirements

### Requirement: Audio Source Enumeration

The system SHALL enumerate available audio output devices for capture.

#### Scenario: List audio sources on Linux

- **WHEN** the application starts on Linux
- **THEN** the system SHALL query available audio output devices via PipeWire
- **AND** the system SHALL query available audio input devices (microphones) via PipeWire
- **AND** each device SHALL have a unique identifier, display name, and source type

#### Scenario: Refresh audio sources

- **WHEN** the user opens the audio source dropdown
- **THEN** the system SHALL refresh the list of available audio devices
- **AND** previously disconnected devices SHALL be removed
- **AND** newly connected devices SHALL appear

#### Scenario: No audio sources available

- **WHEN** no audio output devices are detected
- **THEN** the audio source dropdown SHALL display "No audio sources found"
- **AND** audio recording SHALL be unavailable

#### Scenario: Audio enumeration on unsupported platform

- **WHEN** the application runs on macOS
- **THEN** the audio source list SHALL be empty
- **AND** audio recording SHALL be unavailable
