## ADDED Requirements

### Requirement: Transcription Window Display

The system SHALL provide a separate window for displaying live transcription output during recording.

#### Scenario: Window appearance

- **WHEN** the transcription window is displayed
- **THEN** the window SHALL use the same theme colors as the main application window
- **AND** the window SHALL have rounded corners matching the main window style
- **AND** the window SHALL have a semi-transparent border matching the main window style
- **AND** the window SHALL have a close button in the top-right corner

#### Scenario: Window dimensions

- **WHEN** the transcription window is first opened
- **THEN** the window SHALL have a default width of 300 pixels
- **AND** the window SHALL have a default height of 600 pixels
- **AND** the window SHALL be positioned near the right edge of the primary display

#### Scenario: Window interaction

- **WHEN** the transcription window is displayed
- **THEN** the user SHALL be able to resize the window
- **AND** the user SHALL be able to move the window
- **AND** the user SHALL be able to close the window via the close button
- **AND** the window SHALL NOT block interaction with the main application window

### Requirement: Live Segment Display

The system SHALL display transcription segments in real-time as they are generated.

#### Scenario: Segment append

- **WHEN** a transcription segment is produced during recording
- **THEN** the segment SHALL be appended to the transcription window
- **AND** the segment SHALL display a timestamp prefix in the format `[HH:MM:SS]`
- **AND** the segment text SHALL follow the timestamp on the same line

#### Scenario: Auto-scroll

- **WHEN** a new segment is appended to the transcription window
- **AND** the user has not manually scrolled up
- **THEN** the window SHALL automatically scroll to show the newest segment

#### Scenario: Manual scroll preserved

- **WHEN** the user manually scrolls up in the transcription window
- **AND** a new segment is appended
- **THEN** the scroll position SHALL remain where the user scrolled
- **AND** the new segment SHALL still be appended at the bottom

#### Scenario: Content cleared on new recording

- **WHEN** a new recording starts with transcription enabled
- **THEN** any previous transcript content in the window SHALL be cleared
- **AND** the window SHALL display only segments from the current recording

### Requirement: Automatic Window Opening

The system SHALL automatically open the transcription window when recording starts, based on configuration.

#### Scenario: Auto-open enabled

- **WHEN** recording starts
- **AND** transcription is enabled
- **AND** the "Show transcript when recording starts" setting is enabled
- **THEN** the transcription window SHALL automatically open

#### Scenario: Auto-open disabled

- **WHEN** recording starts
- **AND** transcription is enabled
- **AND** the "Show transcript when recording starts" setting is disabled
- **THEN** the transcription window SHALL NOT automatically open

#### Scenario: Transcription disabled

- **WHEN** recording starts
- **AND** transcription is disabled
- **THEN** the transcription window SHALL NOT automatically open
- **AND** the "Show transcript when recording starts" setting SHALL have no effect

### Requirement: Window Persistence After Recording

The system SHALL allow the transcription window to remain open after recording stops.

#### Scenario: Recording stops with window open

- **WHEN** recording stops
- **AND** the transcription window is open
- **THEN** the transcription window SHALL remain open
- **AND** the transcription content SHALL remain visible for review

#### Scenario: Manual close after recording

- **WHEN** the user closes the transcription window after recording stops
- **THEN** the window SHALL close
- **AND** the transcript content SHALL be cleared from memory

### Requirement: Theme Synchronization

The system SHALL synchronize the transcription window theme with the main application theme.

#### Scenario: Initial theme

- **WHEN** the transcription window opens
- **THEN** the window SHALL use the same theme (light or dark) as the main application window

#### Scenario: Theme change during recording

- **WHEN** the application theme changes while the transcription window is open
- **THEN** the transcription window theme SHALL update to match the new theme
