# gnome-tray-mode Specification

## Purpose
Provides tray-based recording workflow for desktop environments that don't support custom portal pickers (GNOME, KDE Plasma). Users interact via system tray icon and native portal dialogs.
## Requirements
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

#### Scenario: Non-tray-mode Linux environment

- **WHEN** the application starts on Linux
- **AND** the `XDG_CURRENT_DESKTOP` environment variable does not contain "GNOME" or "KDE"
- **THEN** the application SHALL use the standard Hyprland/portal workflow
- **AND** the main window SHALL be shown on startup

### Requirement: System Tray Icon

The system SHALL display a tray icon that provides recording controls.

#### Scenario: Tray icon displayed on startup

- **WHEN** tray mode is active
- **THEN** a system tray icon SHALL appear in the system tray / app indicator area
- **AND** the icon SHALL use the OmniRec application icon

#### Scenario: Tray icon indicates idle state

- **WHEN** no recording is in progress
- **THEN** the tray icon SHALL display the normal OmniRec icon

#### Scenario: Tray icon indicates recording state

- **WHEN** a recording is in progress
- **THEN** the tray icon SHALL change to a red dot icon
- **AND** the icon SHALL remain red until recording stops

#### Scenario: Tray icon removed on exit

- **WHEN** the user selects Exit from the tray menu
- **THEN** the tray icon SHALL be removed from the system tray
- **AND** the application SHALL quit completely

### Requirement: Tray Menu Structure

The tray icon SHALL display a context menu with recording controls and navigation options.

#### Scenario: Tray menu contents

- **WHEN** the user clicks the tray icon
- **THEN** a menu SHALL appear with the following items in order:
  - Start Recording
  - Stop Recording
  - Configuration
  - About
  - Exit

#### Scenario: Start Recording menu item

- **WHEN** the user clicks "Start Recording"
- **AND** no recording is currently in progress
- **THEN** the system SHALL invoke the screen recording portal
- **AND** the desktop's native picker dialog SHALL appear
- **AND** recording SHALL begin after the user makes a selection in the picker

#### Scenario: Start Recording disabled during recording

- **WHEN** a recording is in progress
- **THEN** the "Start Recording" menu item SHALL be disabled (grayed out)

#### Scenario: Stop Recording menu item

- **WHEN** the user clicks "Stop Recording"
- **AND** a recording is currently in progress
- **THEN** the recording SHALL stop
- **AND** the file SHALL be saved to the configured output directory
- **AND** the tray icon SHALL return to the normal icon

#### Scenario: Stop Recording disabled when idle

- **WHEN** no recording is in progress
- **THEN** the "Stop Recording" menu item SHALL be disabled (grayed out)

#### Scenario: Configuration menu item

- **WHEN** the user clicks "Configuration"
- **THEN** the main window SHALL be shown
- **AND** the Configuration tab SHALL be active

#### Scenario: About menu item

- **WHEN** the user clicks "About"
- **THEN** the main window SHALL be shown
- **AND** the About tab SHALL be active

#### Scenario: Exit menu item

- **WHEN** the user clicks "Exit"
- **THEN** any active recording SHALL be stopped and saved
- **AND** the application SHALL quit completely
- **AND** the tray icon SHALL be removed

### Requirement: Tray Mode Window Lifecycle

On tray mode desktops, the main window close button SHALL hide the window instead of quitting the application.

#### Scenario: Close button hides window

- **WHEN** tray mode is active
- **AND** the user clicks the window close button
- **THEN** the main window SHALL be hidden
- **AND** the application SHALL continue running
- **AND** the tray icon SHALL remain visible

#### Scenario: Window can be re-shown from tray

- **WHEN** the main window is hidden
- **AND** the user clicks Configuration or About in the tray menu
- **THEN** the main window SHALL be shown
- **AND** the window SHALL be focused

#### Scenario: Exit still quits

- **WHEN** tray mode is active
- **AND** the user selects Exit from the tray menu
- **THEN** the application SHALL quit completely
- **AND** the tray icon SHALL be removed

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

### Requirement: Portal Recording

On tray mode desktops, recording SHALL use the standard xdg-desktop-portal flow with native picker.

#### Scenario: Portal invoked for recording

- **WHEN** Start Recording is selected from the tray menu
- **THEN** the application SHALL call xdg-desktop-portal ScreenCast methods
- **AND** the desktop's native source picker SHALL appear
- **AND** the application SHALL wait for user selection

#### Scenario: User completes portal selection

- **WHEN** the user selects a source in the portal picker
- **AND** clicks the Share/Record button
- **THEN** recording SHALL begin using the portal-provided PipeWire stream
- **AND** the tray icon SHALL change to recording indicator

#### Scenario: User cancels portal picker

- **WHEN** the user cancels the portal picker
- **THEN** recording SHALL NOT start
- **AND** the application SHALL return to idle state
- **AND** the tray icon SHALL remain normal

