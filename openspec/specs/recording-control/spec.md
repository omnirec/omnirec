# recording-control Specification

## Purpose
TBD - created by archiving change add-window-recording. Update Purpose after archive.
## Requirements
### Requirement: Recording State Management

The system SHALL maintain a recording state machine with states: Idle, Recording, and Saving.

#### Scenario: Initial state

- **WHEN** the application starts
- **THEN** the recording state is Idle
- **AND** the record button displays "Record"
- **AND** the capture mode selector defaults to "Window"
- **AND** the window list is enabled for selection

#### Scenario: Transition to Recording state

- **WHEN** the user clicks the record button while in Idle state
- **AND** a valid capture target is selected (window or region)
- **THEN** the state transitions to Recording
- **AND** the record button displays "Stop"
- **AND** the capture mode selector is disabled
- **AND** the window list or region selection UI is disabled

#### Scenario: Transition to Saving state

- **WHEN** the user clicks the stop button while in Recording state
- **THEN** the state transitions to Saving
- **AND** the record button is disabled
- **AND** a "Saving..." indicator is displayed

#### Scenario: Transition back to Idle state

- **WHEN** the recording file has been saved successfully
- **THEN** the state transitions to Idle
- **AND** the user is notified of the saved file location
- **AND** the capture mode selector is re-enabled
- **AND** the appropriate selection UI (window list or region) is re-enabled

### Requirement: Start Recording

The system SHALL begin capturing and encoding video when recording is started, optionally including audio.

#### Scenario: Start recording successfully

- **WHEN** the user initiates recording
- **THEN** frame capture begins from the selected capture target
- **AND** frames are piped to the FFmpeg encoder process
- **AND** a recording duration timer is displayed

#### Scenario: Start recording with invalid window

- **WHEN** the user initiates recording in window mode
- **AND** the selected window is no longer valid
- **THEN** an error message is displayed
- **AND** the state remains Idle
- **AND** the window list is refreshed

#### Scenario: Start recording with invalid region

- **WHEN** the user initiates recording in region mode
- **AND** the selected region's monitor is no longer available
- **THEN** an error message is displayed
- **AND** the state remains Idle
- **AND** the user is prompted to select a new region

#### Scenario: Start recording with audio

- **WHEN** the user initiates recording
- **AND** audio recording is enabled
- **AND** a valid audio source is selected
- **THEN** audio capture begins from the selected source
- **AND** audio samples are piped to the FFmpeg encoder process

#### Scenario: Start recording with audio source unavailable

- **WHEN** the user initiates recording
- **AND** audio recording is enabled
- **AND** the selected audio source is no longer available
- **THEN** recording starts without audio
- **AND** a warning message is displayed indicating audio is unavailable

### Requirement: Stop Recording

The system SHALL stop capture and finalize the video file when recording is stopped.

#### Scenario: Stop recording and save file

- **WHEN** the user stops recording
- **THEN** frame capture stops
- **AND** the FFmpeg process is signaled to finalize the file
- **AND** the output file is written to the Videos folder
- **AND** the filename includes a timestamp (e.g., `recording_2024-01-15_143052.mp4`)

#### Scenario: Display save confirmation

- **WHEN** the file has been saved successfully
- **THEN** a notification shows the file path
- **AND** the user can click to open the containing folder

#### Scenario: Stop recording with audio

- **WHEN** the user stops recording
- **AND** audio was being captured
- **THEN** audio capture stops
- **AND** remaining audio samples are flushed to FFmpeg
- **AND** the final file contains synchronized audio and video

### Requirement: Video Output Format

The system SHALL encode captured frames to H.264 video in an MP4 container, optionally including an audio track.

#### Scenario: Encode to MP4

- **WHEN** frames are being recorded
- **THEN** video is encoded using H.264 codec (libx264)
- **AND** the container format is MP4
- **AND** the frame rate matches the capture rate (30 FPS default)

#### Scenario: Output file is playable

- **WHEN** recording is complete
- **THEN** the output MP4 file is playable in standard video players
- **AND** the file contains valid H.264 video stream
- **AND** the video dimensions match the captured window size

#### Scenario: Encode with audio

- **WHEN** frames are being recorded
- **AND** audio recording is enabled
- **AND** an audio source is selected
- **THEN** audio is encoded using AAC codec
- **AND** the audio track is muxed into the MP4 container
- **AND** audio and video are synchronized

#### Scenario: Encode without audio

- **WHEN** frames are being recorded
- **AND** audio recording is disabled OR no audio source is selected
- **THEN** the output MP4 contains only the video track
- **AND** no audio track is present in the container

### Requirement: Recording UI Feedback

The system SHALL provide clear visual feedback about recording status.

#### Scenario: Display recording indicator

- **WHEN** recording is active
- **THEN** a red recording indicator is visible
- **AND** the elapsed recording time is displayed (MM:SS format)

#### Scenario: Display saving progress

- **WHEN** the recording is being finalized
- **THEN** a "Saving..." message is displayed
- **AND** the UI indicates the operation is in progress

#### Scenario: Display error state

- **WHEN** a recording error occurs
- **THEN** an error message is displayed to the user
- **AND** the system returns to Idle state
- **AND** the user can attempt to record again

### Requirement: Start Display Recording

The system SHALL begin capturing an entire display when recording is started in display mode.

#### Scenario: Start display recording successfully

- **WHEN** the user initiates recording in display mode
- **AND** a display is selected
- **THEN** frame capture begins from the entire selected display
- **AND** frames are piped to the FFmpeg encoder process
- **AND** a recording duration timer is displayed

#### Scenario: Start recording with invalid display

- **WHEN** the user initiates recording in display mode
- **AND** the selected display is no longer available
- **THEN** an error message is displayed
- **AND** the state remains Idle
- **AND** the display list is refreshed

