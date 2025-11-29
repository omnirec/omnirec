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
- **AND** the window list is enabled for selection

#### Scenario: Transition to Recording state

- **WHEN** the user clicks the record button while in Idle state
- **AND** a window is selected
- **THEN** the state transitions to Recording
- **AND** the record button displays "Stop"
- **AND** the window list is disabled

#### Scenario: Transition to Saving state

- **WHEN** the user clicks the stop button while in Recording state
- **THEN** the state transitions to Saving
- **AND** the record button is disabled
- **AND** a "Saving..." indicator is displayed

#### Scenario: Transition back to Idle state

- **WHEN** the recording file has been saved successfully
- **THEN** the state transitions to Idle
- **AND** the user is notified of the saved file location
- **AND** the window list is re-enabled

### Requirement: Start Recording

The system SHALL begin capturing and encoding video when recording is started.

#### Scenario: Start recording successfully

- **WHEN** the user initiates recording
- **THEN** frame capture begins from the selected window
- **AND** frames are piped to the FFmpeg encoder process
- **AND** a recording duration timer is displayed

#### Scenario: Start recording with invalid window

- **WHEN** the user initiates recording
- **AND** the selected window is no longer valid
- **THEN** an error message is displayed
- **AND** the state remains Idle
- **AND** the window list is refreshed

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

### Requirement: Video Output Format

The system SHALL encode captured frames to H.264 video in an MP4 container.

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

