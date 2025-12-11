## ADDED Requirements

### Requirement: Linux Visual Highlight

The system SHALL display a visual highlight border on Linux/Hyprland when previewing capture targets.

#### Scenario: Show window highlight on Linux

- **GIVEN** a window is selected for capture on Linux/Hyprland
- **WHEN** `show_highlight` is called with the window's bounds
- **THEN** a visible border is rendered around the window
- **AND** the highlight is non-interactive (click-through)
- **AND** the highlight auto-dismisses after approximately 800ms

#### Scenario: Show display highlight on Linux

- **GIVEN** a display is selected for capture on Linux/Hyprland
- **WHEN** `show_highlight` is called with the display's bounds
- **THEN** a visible border is rendered around the display edges
- **AND** the highlight is visible above all other windows
- **AND** the highlight auto-dismisses after approximately 800ms

#### Scenario: Highlight visual consistency

- **GIVEN** the highlight is displayed on Linux
- **THEN** the border color is #2196F3 (blue)
- **AND** the border width is approximately 6-8 pixels
- **AND** the appearance is consistent with Windows and macOS highlights

#### Scenario: Rapid successive highlights

- **GIVEN** a highlight is currently visible on Linux
- **WHEN** `show_highlight` is called again with different coordinates
- **THEN** the previous highlight is dismissed
- **AND** a new highlight is shown at the new location
