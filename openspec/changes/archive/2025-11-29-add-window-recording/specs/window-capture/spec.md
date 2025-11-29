## ADDED Requirements

### Requirement: Window Enumeration

The system SHALL provide a list of all visible, capturable windows on the system.

#### Scenario: List available windows

- **WHEN** the user opens the application
- **THEN** a list of capturable windows is displayed
- **AND** each window entry shows the window title
- **AND** each window entry shows the owning application name

#### Scenario: Refresh window list

- **WHEN** the user requests a refresh of the window list
- **THEN** the list is updated with current windows
- **AND** previously selected window selection is cleared if window no longer exists

#### Scenario: Filter system windows

- **WHEN** enumerating windows
- **THEN** system UI windows (taskbar, start menu, etc.) are excluded
- **AND** invisible windows are excluded
- **AND** minimized windows are excluded

### Requirement: Window Selection

The system SHALL allow the user to select a single window from the enumerated list for capture.

#### Scenario: Select window for capture

- **WHEN** the user clicks on a window entry in the list
- **THEN** that window is marked as selected
- **AND** the record button becomes enabled
- **AND** any previously selected window is deselected

#### Scenario: No window selected

- **WHEN** no window is selected
- **THEN** the record button is disabled
- **AND** a prompt indicates the user should select a window

### Requirement: Frame Capture

The system SHALL capture frames from the selected window using Windows.Graphics.Capture API.

#### Scenario: Capture frames during recording

- **WHEN** recording is active
- **THEN** frames are captured at the configured frame rate (default 30 FPS)
- **AND** frames are in BGRA pixel format
- **AND** frames include the window's client area content

#### Scenario: Window resized during capture

- **WHEN** the target window is resized during recording
- **THEN** capture continues with the new window dimensions
- **AND** the output video reflects the dimension change

#### Scenario: Window closed during capture

- **WHEN** the target window is closed during recording
- **THEN** recording stops automatically
- **AND** the partial recording is saved
- **AND** the user is notified that the window was closed
