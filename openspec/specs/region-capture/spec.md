# region-capture Specification

## Purpose
TBD - created by archiving change add-region-capture. Update Purpose after archive.
## Requirements
### Requirement: Display Enumeration

The system SHALL provide a list of all connected displays/monitors for region selection.

#### Scenario: List available monitors

- **WHEN** the user switches to region capture mode
- **THEN** the system enumerates all connected monitors
- **AND** each monitor entry includes its position, dimensions, and scale factor
- **AND** the primary monitor is identified

#### Scenario: Handle monitor disconnection

- **WHEN** a previously selected monitor is disconnected
- **THEN** the current region selection is cleared
- **AND** the user is prompted to select a new region

### Requirement: Region Selection Overlay

The system SHALL display an interactive overlay window for selecting a screen region.

#### Scenario: Open selection overlay

- **WHEN** the user clicks "Select Region" button
- **THEN** a semi-transparent overlay window appears
- **AND** the overlay covers all connected monitors
- **AND** the overlay is always-on-top of other windows
- **AND** a crosshair cursor is displayed

#### Scenario: Draw initial selection rectangle

- **WHEN** the user clicks and drags on the overlay
- **THEN** a selection rectangle is drawn from the click origin to the current cursor position
- **AND** the rectangle dimensions are displayed in pixels
- **AND** the selection rectangle has a visible border

#### Scenario: Constrain selection to single monitor

- **WHEN** the user drags the selection across monitor boundaries
- **THEN** the selection is constrained to the monitor where the drag started
- **AND** the cursor can move freely but the rectangle clips at monitor edges

### Requirement: Selection Rectangle Manipulation

The system SHALL allow the user to resize and reposition the selection rectangle after initial creation.

#### Scenario: Resize selection via corner handles

- **WHEN** the user drags a corner handle of the selection rectangle
- **THEN** the opposite corner remains fixed
- **AND** the rectangle resizes proportionally to cursor movement
- **AND** the updated dimensions are displayed

#### Scenario: Resize selection via edge handles

- **WHEN** the user drags an edge handle of the selection rectangle
- **THEN** the opposite edge remains fixed
- **AND** only the corresponding dimension (width or height) changes

#### Scenario: Reposition selection via center drag

- **WHEN** the user drags the center area of the selection rectangle
- **THEN** the entire rectangle moves with the cursor
- **AND** the rectangle dimensions remain unchanged
- **AND** the rectangle is constrained to the current monitor boundaries

#### Scenario: Enforce minimum selection size

- **WHEN** the user attempts to resize the selection below 100x100 pixels
- **THEN** the selection size is clamped to the minimum dimensions
- **AND** visual feedback indicates the minimum has been reached

### Requirement: Selection Confirmation

The system SHALL allow the user to confirm or cancel the region selection.

#### Scenario: Confirm region selection

- **WHEN** the user clicks the "Confirm" button or presses Enter
- **THEN** the overlay window closes
- **AND** the selected region is stored for recording
- **AND** the main window displays the selected region dimensions

#### Scenario: Cancel region selection

- **WHEN** the user clicks the "Cancel" button or presses Escape
- **THEN** the overlay window closes
- **AND** any previous region selection is preserved
- **AND** if no previous selection existed, the Record button remains disabled

### Requirement: Region Frame Capture

The system SHALL capture frames from the selected screen region.

#### Scenario: Capture frames from region during recording

- **WHEN** recording is active in region mode
- **THEN** frames are captured from the monitor containing the region
- **AND** frames are cropped to the selected region boundaries
- **AND** the output frame dimensions match the region dimensions
- **AND** frames are captured at the configured frame rate (default 30 FPS)

#### Scenario: Selected region changes during capture

- **WHEN** the monitor resolution changes during recording
- **THEN** recording continues with the original region coordinates
- **AND** if the region extends beyond new boundaries, it is clipped
- **AND** the user is notified if the region became invalid

### Requirement: Capture Mode Selection

The system SHALL allow the user to switch between window capture, region capture, and display capture modes.

#### Scenario: Switch to region capture mode

- **WHEN** the user selects "Region" in the capture mode selector
- **THEN** the window list is hidden
- **AND** the display selection UI is hidden
- **AND** the "Select Region" button is displayed
- **AND** any current window selection is cleared

#### Scenario: Switch to window capture mode

- **WHEN** the user selects "Window" in the capture mode selector
- **THEN** the window list is displayed
- **AND** the region selection UI is hidden
- **AND** the display selection UI is hidden
- **AND** any current region selection is preserved (for later use)

#### Scenario: Switch to display capture mode

- **WHEN** the user selects "Display" in the capture mode selector
- **THEN** the display selection UI is displayed
- **AND** the window list is hidden
- **AND** the region selection UI is hidden
- **AND** any current window selection is cleared

#### Scenario: Display selected region info

- **WHEN** a region is selected and the mode is "Region"
- **THEN** the selected region dimensions are displayed (e.g., "1920 x 1080")
- **AND** the monitor name is displayed
- **AND** the Record button is enabled

