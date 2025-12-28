## ADDED Requirements

### Requirement: Cinnamon Wayland Detection

The system SHALL detect Cinnamon Wayland sessions and activate tray mode.

#### Scenario: Cinnamon detected via XDG_CURRENT_DESKTOP

- **WHEN** the application starts on Linux
- **AND** the `XDG_CURRENT_DESKTOP` environment variable contains "X-CINNAMON"
- **THEN** the application SHALL activate tray mode
- **AND** the application SHALL add a system tray icon
- **AND** the main window SHALL NOT be shown on startup

#### Scenario: Cinnamon uses portal picker

- **WHEN** tray mode is active on Cinnamon Wayland
- **AND** the user clicks "Record Screen/Window" from the tray menu
- **THEN** the xdg-desktop-portal ScreenCast flow SHALL be invoked
- **AND** Cinnamon's native picker dialog SHALL appear for source selection

#### Scenario: Capture tabs hidden on Cinnamon

- **WHEN** tray mode is active on Cinnamon Wayland
- **AND** the main window is displayed
- **THEN** the Window tab SHALL NOT be visible
- **AND** the Region tab SHALL NOT be visible
- **AND** the Display tab SHALL NOT be visible
- **AND** the Configuration tab SHALL be visible
- **AND** the About tab SHALL be visible

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

#### Scenario: Cinnamon detected via XDG_CURRENT_DESKTOP

- **WHEN** the application starts on Linux
- **AND** the `XDG_CURRENT_DESKTOP` environment variable contains "X-CINNAMON"
- **THEN** the application SHALL activate tray mode
- **AND** the application SHALL add a system tray icon
- **AND** the main window SHALL NOT be shown on startup

#### Scenario: Non-tray-mode Linux environment

- **WHEN** the application starts on Linux
- **AND** the `XDG_CURRENT_DESKTOP` environment variable does not contain "GNOME", "KDE", "COSMIC", or "X-CINNAMON"
- **THEN** the application SHALL use the standard Hyprland/portal workflow
- **AND** the main window SHALL be shown on startup
