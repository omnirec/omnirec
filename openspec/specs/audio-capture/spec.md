# audio-capture Specification

## Purpose
TBD - created by archiving change add-audio-recording. Update Purpose after archive.
## Requirements
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

### Requirement: Audio Capture Backend Trait

The system SHALL define a platform-agnostic trait for audio capture operations.

#### Scenario: Start audio capture

- **WHEN** recording starts with audio enabled and source(s) selected
- **THEN** the audio capture backend SHALL begin capturing PCM samples from selected source(s)
- **AND** if both sources selected, samples SHALL be mixed before sending to encoder
- **AND** samples SHALL be sent to a channel for the encoder

#### Scenario: Stop audio capture

- **WHEN** recording stops
- **THEN** the audio capture backend SHALL stop capturing from all active sources
- **AND** remaining buffered samples SHALL be flushed

#### Scenario: Audio source disconnection during recording

- **WHEN** a selected audio source disconnects during recording
- **THEN** the audio capture for that source SHALL stop gracefully
- **AND** if another audio source is still active, recording SHALL continue with that source
- **AND** if both sources disconnect, video recording SHALL continue
- **AND** the user SHALL be notified that audio was lost

### Requirement: Audio Sample Format

The system SHALL capture audio in a consistent format across platforms.

#### Scenario: Audio capture format

- **WHEN** audio capture is active
- **THEN** samples SHALL be 48kHz sample rate
- **AND** samples SHALL be stereo (2 channels)
- **AND** samples SHALL be 16-bit signed integer PCM

### Requirement: Linux Audio Capture

The system SHALL capture system audio on Linux using PipeWire.

#### Scenario: PipeWire audio capture

- **WHEN** audio capture starts on Linux
- **THEN** the system SHALL create a PipeWire stream for audio capture
- **AND** the stream SHALL capture from the selected audio output device

#### Scenario: PipeWire not available

- **WHEN** PipeWire is not running or unavailable
- **THEN** audio enumeration SHALL return an empty list
- **AND** audio recording SHALL be unavailable

### Requirement: macOS Audio Capture Stub

The system SHALL provide a stub implementation for macOS audio capture.

#### Scenario: macOS audio capture not implemented

- **WHEN** audio capture is requested on macOS
- **THEN** the system SHALL return a `NotImplemented` error
- **AND** audio recording SHALL be unavailable

### Requirement: Dual Audio Source Capture

The system SHALL support capturing audio from two sources simultaneously: a system audio source (output monitor) and a microphone input.

#### Scenario: Capture system audio and microphone together

- **WHEN** recording starts with both system audio source and microphone selected
- **THEN** the system SHALL capture audio from both sources concurrently
- **AND** both audio streams SHALL be synchronized by timestamp

#### Scenario: Capture system audio only

- **WHEN** recording starts with system audio source selected and no microphone
- **THEN** the system SHALL capture only the system audio source
- **AND** behavior SHALL match existing single-source capture

#### Scenario: Capture microphone only

- **WHEN** recording starts with microphone selected and no system audio source
- **THEN** the system SHALL capture only the microphone input
- **AND** the microphone audio SHALL be included in the output recording

### Requirement: Audio Stream Mixing

The system SHALL mix multiple audio streams into a single output stream before encoding.

#### Scenario: Mix system audio and microphone

- **WHEN** both system audio and microphone are being captured
- **THEN** the system SHALL mix the two streams into a single stereo stream
- **AND** the mixing SHALL use equal weighting (0.5 for each source)
- **AND** the mixed output SHALL be clamped to prevent clipping

#### Scenario: Handle mono microphone input

- **WHEN** the microphone provides mono audio
- **THEN** the system SHALL convert mono to stereo by duplicating the channel
- **AND** the stereo output SHALL be mixed with the system audio

#### Scenario: Handle stream timing differences

- **WHEN** system audio and microphone streams have different latencies
- **THEN** the system SHALL buffer and align samples by timestamp
- **AND** minor timing drift SHALL be handled by resampling or sample dropping

### Requirement: Acoustic Echo Cancellation

The system SHALL provide optional acoustic echo cancellation (AEC) for microphone input to remove speaker audio picked up by the microphone.

#### Scenario: AEC enabled with both sources

- **WHEN** AEC is enabled and both system audio and microphone are selected
- **THEN** the system SHALL use the system audio stream as the AEC reference signal
- **AND** the microphone input SHALL be processed through the AEC filter before mixing
- **AND** speaker audio picked up by the microphone SHALL be attenuated

#### Scenario: AEC disabled

- **WHEN** AEC is disabled
- **THEN** the microphone input SHALL be mixed directly without echo cancellation processing

#### Scenario: AEC initialization failure

- **WHEN** AEC is enabled but AEC3 initialization fails
- **THEN** the system SHALL log a warning
- **AND** recording SHALL proceed without echo cancellation
- **AND** the user SHALL NOT receive an error

#### Scenario: AEC with only microphone selected

- **WHEN** AEC is enabled but no system audio source is selected
- **THEN** AEC SHALL be skipped (no reference signal available)
- **AND** recording SHALL proceed with microphone only

### Requirement: Linux Dual Audio Capture

The system SHALL implement dual audio source capture on Linux using PipeWire.

#### Scenario: Create two capture streams on Linux

- **WHEN** recording starts with both sources selected on Linux
- **THEN** the system SHALL create a PipeWire stream for the system audio output (sink monitor)
- **AND** the system SHALL create a second PipeWire stream for the microphone input
- **AND** both streams SHALL deliver samples to the audio mixer

#### Scenario: AEC3 processing on Linux

- **WHEN** AEC is enabled on Linux with both sources active
- **THEN** the system SHALL process microphone samples through the aec3 crate (WebRTC AEC3 port)
- **AND** the system SHALL use system audio as the render (reference) signal
- **AND** the system SHALL process samples in 10ms frames (480 samples per channel at 48kHz)

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

