# system-tray Specification

## Purpose
TBD - created by archiving change refactor-tray-abstraction. Update Purpose after archive.
## Requirements
### Requirement: Tray Icon Initialization

The system SHALL initialize a system tray icon on all supported platforms.

#### Scenario: Tray icon created on application startup

- **WHEN** the application starts
- **THEN** a system tray icon SHALL be created
- **AND** the icon SHALL appear in the platform's tray/notification area
- **AND** the application icon SHALL be used for the tray icon

#### Scenario: Tray initialization fails gracefully

- **WHEN** the application starts
- **AND** tray creation fails (unsupported environment, permissions, etc.)
- **THEN** the application SHALL continue running without tray
- **AND** an error message SHALL be logged

### Requirement: Tray Menu Structure

The tray icon SHALL display a context menu with recording controls and navigation options.

#### Scenario: Tray menu contents

- **WHEN** the user clicks or right-clicks the tray icon
- **THEN** a menu SHALL appear with the following items:
  - Start Recording
  - Stop Recording
  - Configuration
  - About
  - Exit

#### Scenario: Start Recording menu item invokes recording

- **WHEN** the user clicks "Start Recording"
- **AND** no recording is currently in progress
- **THEN** the recording workflow SHALL be initiated
- **AND** platform-specific source selection SHALL occur

#### Scenario: Start Recording disabled during recording

- **WHEN** a recording is in progress
- **THEN** the "Start Recording" menu item SHALL be disabled (grayed out)

#### Scenario: Stop Recording menu item stops recording

- **WHEN** the user clicks "Stop Recording"
- **AND** a recording is currently in progress
- **THEN** the recording SHALL stop
- **AND** the file SHALL be saved to the configured output directory

#### Scenario: Stop Recording disabled when idle

- **WHEN** no recording is in progress
- **THEN** the "Stop Recording" menu item SHALL be disabled (grayed out)

#### Scenario: Configuration menu item shows settings

- **WHEN** the user clicks "Configuration"
- **THEN** the main window SHALL be shown
- **AND** the Configuration tab SHALL be active

#### Scenario: About menu item shows about

- **WHEN** the user clicks "About"
- **THEN** the main window SHALL be shown
- **AND** the About tab SHALL be active

#### Scenario: Exit menu item quits application

- **WHEN** the user clicks "Exit"
- **THEN** any active recording SHALL be stopped and saved
- **AND** the application SHALL quit completely
- **AND** the tray icon SHALL be removed

### Requirement: Tray Recording State Indicator

The tray icon SHALL visually indicate when a recording is in progress.

#### Scenario: Tray icon indicates idle state

- **WHEN** no recording is in progress
- **THEN** the tray icon SHALL display the normal application icon

#### Scenario: Tray icon indicates recording state

- **WHEN** a recording is in progress
- **THEN** the tray icon SHALL change to a recording indicator (red dot icon)
- **AND** the indicator SHALL remain until recording stops

#### Scenario: Tray icon returns to idle after recording

- **WHEN** a recording is stopped
- **THEN** the tray icon SHALL return to the normal application icon

### Requirement: Tray Visibility Control

The system SHALL provide programmatic control over tray icon visibility.

#### Scenario: Hide tray icon

- **WHEN** `set_tray_visible(false)` is called
- **THEN** the tray icon SHALL be hidden from the system tray

#### Scenario: Show tray icon

- **WHEN** `set_tray_visible(true)` is called
- **THEN** the tray icon SHALL be visible in the system tray

### Requirement: Platform Module Organization

The tray implementation SHALL be organized into platform-specific modules with compile-time selection.

#### Scenario: Windows platform module

- **GIVEN** the application is compiled for Windows
- **WHEN** the tray module is loaded
- **THEN** the `tray/windows.rs` module SHALL be compiled and used

#### Scenario: macOS platform module

- **GIVEN** the application is compiled for macOS
- **WHEN** the tray module is loaded
- **THEN** the `tray/macos.rs` module SHALL be compiled and used

#### Scenario: Linux platform module

- **GIVEN** the application is compiled for Linux
- **WHEN** the tray module is loaded
- **THEN** the `tray/linux.rs` module SHALL be compiled and used

### Requirement: Stub Implementations for Unsupported Platforms

Platforms without full tray implementation SHALL provide stub implementations that succeed without creating actual tray icons.

#### Scenario: macOS stub implementation

- **GIVEN** the macOS tray implementation is a stub
- **WHEN** tray setup is called
- **THEN** the function SHALL return success
- **AND** a debug message SHALL be logged indicating stub mode
- **AND** no actual tray icon SHALL be created

### Requirement: Windows Tray Implementation

The system SHALL provide full tray functionality on Windows, matching the behavior of the Linux implementation.

#### Scenario: Windows tray icon created on startup

- **WHEN** the application starts on Windows
- **THEN** a system tray icon SHALL be created in the notification area
- **AND** the icon SHALL use the OmniRec application icon

#### Scenario: Windows tray menu available

- **WHEN** the user right-clicks the Windows tray icon
- **THEN** a context menu SHALL appear with the following items:
  - Start Recording
  - Stop Recording
  - Configuration
  - About
  - Exit

#### Scenario: Windows tray icon indicates recording state

- **WHEN** a recording is in progress on Windows
- **THEN** the tray icon SHALL change to a recording indicator (red dot icon)
- **AND** the indicator SHALL remain until recording stops

#### Scenario: Windows tray icon returns to idle after recording

- **WHEN** a recording is stopped on Windows
- **THEN** the tray icon SHALL return to the normal application icon

