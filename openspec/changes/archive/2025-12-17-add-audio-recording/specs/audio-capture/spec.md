# audio-capture Specification

## Purpose

Provides system audio capture functionality for recording desktop audio output alongside screen recordings. This change implements Linux support only; Windows and macOS have stub implementations.

## ADDED Requirements

### Requirement: Audio Source Enumeration

The system SHALL enumerate available audio output devices for capture.

#### Scenario: List audio sources on Linux

- **WHEN** the application starts on Linux
- **THEN** the system SHALL query available audio output devices via PipeWire
- **AND** each device SHALL have a unique identifier and display name

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

- **WHEN** the application runs on Windows or macOS
- **THEN** the audio source list SHALL be empty
- **AND** audio recording SHALL be unavailable

### Requirement: Audio Capture Backend Trait

The system SHALL define a platform-agnostic trait for audio capture operations.

#### Scenario: Start audio capture

- **WHEN** recording starts with audio enabled and a source selected
- **THEN** the audio capture backend SHALL begin capturing PCM samples
- **AND** samples SHALL be sent to a channel for the encoder

#### Scenario: Stop audio capture

- **WHEN** recording stops
- **THEN** the audio capture backend SHALL stop capturing
- **AND** remaining buffered samples SHALL be flushed

#### Scenario: Audio source disconnection during recording

- **WHEN** the selected audio source disconnects during recording
- **THEN** the audio capture SHALL stop gracefully
- **AND** video recording SHALL continue
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

### Requirement: Windows Audio Capture Stub

The system SHALL provide a stub implementation for Windows audio capture.

#### Scenario: Windows audio capture not implemented

- **WHEN** audio capture is requested on Windows
- **THEN** the system SHALL return a `NotImplemented` error
- **AND** audio recording SHALL be unavailable

### Requirement: macOS Audio Capture Stub

The system SHALL provide a stub implementation for macOS audio capture.

#### Scenario: macOS audio capture not implemented

- **WHEN** audio capture is requested on macOS
- **THEN** the system SHALL return a `NotImplemented` error
- **AND** audio recording SHALL be unavailable
