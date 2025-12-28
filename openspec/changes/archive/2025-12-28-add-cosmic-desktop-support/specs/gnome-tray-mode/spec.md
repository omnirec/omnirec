## MODIFIED Requirements

### Requirement: Tray Mode Desktop Detection

The system SHALL detect when running on a tray-mode desktop environment and activate tray mode.

#### Scenario: GNOME detected via XDG_CURRENT_DESKTOP

- **WHEN** the application starts on Linux
- **AND** the `XDG_CURRENT_DESKTOP` environment variable contains "GNOME"
- **THEN** the application SHALL activate tray mode
- **AND** the application SHALL add a system tray icon
- **AND** the main window SHALL NOT be shown on startup

#### Scenario: KDE Plasma detected via XDG_CURRENT_DESKTOP

- **WHEN** the application starts on Linux
- **AND** the `XDG_CURRENT_DESKTOP` environment variable contains "KDE"
- **THEN** the application SHALL activate tray mode
- **AND** the application SHALL add a system tray icon
- **AND** the main window SHALL NOT be shown on startup

#### Scenario: COSMIC detected via XDG_CURRENT_DESKTOP

- **WHEN** the application starts on Linux
- **AND** the `XDG_CURRENT_DESKTOP` environment variable contains "COSMIC"
- **THEN** the application SHALL activate tray mode
- **AND** the application SHALL add a system tray icon
- **AND** the main window SHALL NOT be shown on startup

#### Scenario: Non-tray-mode Linux environment

- **WHEN** the application starts on Linux
- **AND** the `XDG_CURRENT_DESKTOP` environment variable does not contain "GNOME", "KDE", or "COSMIC"
- **THEN** the application SHALL use the standard Hyprland/portal workflow
- **AND** the main window SHALL be shown on startup

## MODIFIED Requirements

### Requirement: Tray Mode Tab Visibility

On tray mode desktops (GNOME, KDE, COSMIC), capture mode tabs SHALL be hidden since the portal handles source selection.

#### Scenario: Capture tabs hidden on GNOME

- **WHEN** tray mode is active on GNOME
- **AND** the main window is displayed
- **THEN** the Window tab SHALL NOT be visible
- **AND** the Region tab SHALL NOT be visible
- **AND** the Display tab SHALL NOT be visible

#### Scenario: Capture tabs hidden on KDE

- **WHEN** tray mode is active on KDE Plasma
- **AND** the main window is displayed
- **THEN** the Window tab SHALL NOT be visible
- **AND** the Region tab SHALL NOT be visible
- **AND** the Display tab SHALL NOT be visible

#### Scenario: Capture tabs hidden on COSMIC

- **WHEN** tray mode is active on COSMIC
- **AND** the main window is displayed
- **THEN** the Window tab SHALL NOT be visible
- **AND** the Region tab SHALL NOT be visible
- **AND** the Display tab SHALL NOT be visible

#### Scenario: Config and About tabs visible on tray mode desktops

- **WHEN** tray mode is active (GNOME, KDE, or COSMIC)
- **AND** the main window is displayed
- **THEN** the Configuration tab SHALL be visible
- **AND** the About tab SHALL be visible

#### Scenario: All tabs visible on non-tray-mode desktops

- **WHEN** tray mode is NOT active
- **THEN** all tabs (Window, Region, Display, Config, About) SHALL be visible
