## ADDED Requirements

### Requirement: macOS Tray Implementation

The system SHALL provide full tray functionality on macOS using the system menu bar.

#### Scenario: macOS tray icon created on startup

- **WHEN** the application starts on macOS
- **THEN** a menu bar icon SHALL be created in the system menu bar
- **AND** the icon SHALL use a monochromatic template icon

#### Scenario: macOS icon adapts to system theme

- **WHEN** the macOS menu bar uses light appearance
- **THEN** the tray icon SHALL appear dark (automatically tinted by macOS)

- **WHEN** the macOS menu bar uses dark appearance
- **THEN** the tray icon SHALL appear light (automatically tinted by macOS)

#### Scenario: macOS tray menu available

- **WHEN** the user clicks the macOS menu bar icon
- **THEN** a menu SHALL appear with the following items:
  - Record Screen/Window
  - Stop Recording
  - Transcription
  - Configuration
  - About
  - Exit

#### Scenario: macOS Record Screen/Window shows main window

- **WHEN** the user clicks "Record Screen/Window" on macOS
- **THEN** the main application window SHALL be shown
- **AND** the main window SHALL be activated (brought to front)

#### Scenario: macOS tray icon indicates recording state

- **WHEN** a recording is in progress on macOS
- **THEN** the tray icon SHALL change to a recording indicator with a red dot
- **AND** the indicator SHALL use a template icon that adapts to menu bar appearance

#### Scenario: macOS tray icon returns to idle after recording

- **WHEN** a recording is stopped on macOS
- **THEN** the tray icon SHALL return to the normal template icon

### Requirement: Transcription Menu Item

The tray menu SHALL include a Transcription item for accessing the live transcription view.

#### Scenario: Transcription menu item in tray menu

- **WHEN** the user opens the tray menu
- **THEN** a "Transcription" menu item SHALL appear between "Stop Recording" and "Configuration"

#### Scenario: Transcription item shows transcription window when active

- **WHEN** the user clicks "Transcription"
- **AND** transcription is currently active (recording with transcription enabled)
- **THEN** the transcription window SHALL be shown
- **AND** the transcription window SHALL be activated (brought to front)

#### Scenario: Transcription item shows error when not active

- **WHEN** the user clicks "Transcription"
- **AND** transcription is NOT currently active
- **THEN** an error message SHALL be displayed indicating transcription is not active

## MODIFIED Requirements

### Requirement: Tray Menu Structure

The tray icon SHALL display a context menu with recording controls, transcription access, and navigation options.

#### Scenario: Tray menu contents

- **WHEN** the user clicks or right-clicks the tray icon
- **THEN** a menu SHALL appear with the following items:
  - Record Screen/Window (macOS) / Start Recording (Linux/Windows)
  - Stop Recording
  - Transcription
  - Configuration
  - About
  - Exit

#### Scenario: Start Recording menu item invokes recording

- **WHEN** the user clicks "Start Recording" (Linux/Windows)
- **AND** no recording is currently in progress
- **THEN** the recording workflow SHALL be initiated
- **AND** platform-specific source selection SHALL occur

#### Scenario: Record Screen/Window shows main window (macOS)

- **WHEN** the user clicks "Record Screen/Window" (macOS)
- **THEN** the main application window SHALL be shown
- **AND** the main window SHALL be activated (brought to front)

#### Scenario: Start Recording disabled during recording

- **WHEN** a recording is in progress
- **THEN** the "Start Recording" or "Record Screen/Window" menu item SHALL be disabled (grayed out)

#### Scenario: Stop Recording menu item stops recording

- **WHEN** the user clicks "Stop Recording"
- **AND** a recording is currently in progress
- **THEN** the recording SHALL stop
- **AND** the file SHALL be saved to the configured output directory

#### Scenario: Stop Recording disabled when idle

- **WHEN** no recording is in progress
- **THEN** the "Stop Recording" menu item SHALL be disabled (grayed out)

#### Scenario: Transcription menu item shows transcription

- **WHEN** the user clicks "Transcription"
- **AND** transcription is currently active
- **THEN** the transcription window SHALL be shown and activated

#### Scenario: Transcription disabled when not available

- **WHEN** transcription is NOT currently active
- **AND** the user clicks "Transcription"
- **THEN** an error message SHALL be displayed

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

### Requirement: Stub Implementations for Unsupported Platforms

Platforms without full tray implementation SHALL provide stub implementations that succeed without creating actual tray icons.

#### Scenario: All platforms have full implementations

- **GIVEN** macOS, Linux, and Windows all have full tray implementations
- **THEN** no stub implementations are required
- **AND** all platforms SHALL create functional tray icons

## REMOVED Requirements

### Requirement: macOS stub implementation

**Reason**: macOS now has a full tray implementation, so the stub is no longer needed.

**Migration**: Replace stub with full implementation in `src-tauri/src/tray/macos.rs`.
