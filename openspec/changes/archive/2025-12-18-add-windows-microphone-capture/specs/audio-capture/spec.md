## ADDED Requirements

### Requirement: Windows Dual Audio Capture

The system SHALL implement dual audio source capture on Windows using WASAPI.

#### Scenario: Create two capture threads on Windows

- **WHEN** recording starts with both system audio and microphone sources selected on Windows
- **THEN** the system SHALL spawn a WASAPI loopback capture thread for the system audio output
- **AND** the system SHALL spawn a WASAPI direct capture thread for the microphone input
- **AND** both threads SHALL deliver samples to a shared audio mixer

#### Scenario: Audio mixing on Windows

- **WHEN** both capture threads are delivering samples
- **THEN** the system SHALL buffer samples from both streams
- **AND** the system SHALL mix samples using equal weighting (0.5 for each source)
- **AND** the mixed output SHALL be clamped to prevent clipping
- **AND** the mixed samples SHALL be delivered to the encoder

#### Scenario: AEC3 processing on Windows

- **WHEN** AEC is enabled on Windows with both sources active
- **THEN** the system SHALL process microphone samples through the aec3 crate (WebRTC AEC3 port)
- **AND** the system SHALL use system audio as the render (reference) signal
- **AND** the system SHALL process samples in 10ms frames (480 samples per channel at 48kHz)

#### Scenario: Single source fallback on Windows

- **WHEN** only one audio source is specified on Windows
- **THEN** the system SHALL capture using the existing single-source WASAPI capture
- **AND** no mixing or AEC processing SHALL be performed

#### Scenario: Graceful stop with dual capture on Windows

- **WHEN** the stop handle is set during dual capture on Windows
- **THEN** both WASAPI capture threads SHALL stop capturing
- **AND** the mixer thread SHALL flush remaining buffered samples
- **AND** all threads SHALL terminate gracefully
- **AND** all WASAPI resources SHALL be released
