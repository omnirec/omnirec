## MODIFIED Requirements

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
