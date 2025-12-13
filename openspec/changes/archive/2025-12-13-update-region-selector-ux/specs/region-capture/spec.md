## MODIFIED Requirements

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

#### Scenario: Dimensions display auto-hide

- **WHEN** the user moves or resizes the selection rectangle
- **THEN** the dimensions indicator becomes visible
- **AND** after 1.5 seconds of inactivity, the dimensions indicator fades out
- **WHEN** the user moves or resizes again before the timeout
- **THEN** the timeout is reset and dimensions remain visible

#### Scenario: Restore previous selector position

- **WHEN** the user clicks "Change Region" after previously closing the selector
- **AND** a region was previously selected
- **THEN** the selector window opens at the previously stored position and size
- **WHEN** no previous position exists
- **THEN** the selector opens centered on the primary monitor with default dimensions

## ADDED Requirements

### Requirement: Region Selector Lifecycle Management

The system SHALL manage the region selector window lifecycle to prevent orphaned windows and improve workflow.

#### Scenario: Close selector on recording complete

- **WHEN** a region recording completes successfully
- **AND** the region selector window is open
- **THEN** the selector window is automatically closed
- **AND** the selected region remains stored for future recordings

#### Scenario: Close selector on main window close

- **WHEN** the main application window is closed
- **AND** the region selector window is open
- **THEN** the selector window is automatically closed

#### Scenario: Persist selector geometry on close

- **WHEN** the region selector window is closed (manually or automatically)
- **THEN** the current position and size are stored
- **AND** the stored geometry persists for the session
