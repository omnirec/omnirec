# display-capture Specification

## Purpose

Enable users to record an entire display/monitor without needing to manually select a region covering the full screen.

## ADDED Requirements

### Requirement: Display Selection UI

The system SHALL provide a UI section for selecting a display to record when in Display capture mode.

#### Scenario: Display mode UI visible

- **WHEN** the user selects "Display" in the capture mode selector
- **THEN** a display selection section is shown
- **AND** the window list is hidden
- **AND** the region selection UI is hidden

#### Scenario: Display list populated

- **WHEN** the display selection section is shown
- **THEN** a dropdown or list displays all connected monitors
- **AND** each entry shows the monitor name and resolution (e.g., "Display 1 (1920 x 1080)")
- **AND** the primary monitor is indicated

### Requirement: Display Selection

The system SHALL allow the user to select a display from the enumerated list for full-screen capture.

#### Scenario: Select display for capture

- **WHEN** the user selects a display from the list
- **THEN** that display is marked as selected
- **AND** the Record button becomes enabled
- **AND** the status displays the selected display name

#### Scenario: No display selected

- **WHEN** no display is selected in Display mode
- **THEN** the Record button is disabled
- **AND** a prompt indicates the user should select a display

#### Scenario: Display disconnected during selection

- **WHEN** the selected display is disconnected
- **THEN** the selection is cleared
- **AND** the display list is refreshed
- **AND** the user is prompted to select a new display

### Requirement: Display Frame Capture

The system SHALL capture frames from the entire selected display.

#### Scenario: Capture frames from display during recording

- **WHEN** recording is active in display mode
- **THEN** frames are captured from the entire selected monitor
- **AND** the output frame dimensions match the display resolution
- **AND** frames are captured at the configured frame rate (default 30 FPS)
- **AND** frames include the cursor

#### Scenario: Display resolution changes during capture

- **WHEN** the display resolution changes during recording
- **THEN** capture adapts to the new resolution
- **AND** the output video reflects the resolution change
- **AND** the user is notified of the resolution change
