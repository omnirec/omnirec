# platform-abstraction Specification

## Purpose
TBD - created by archiving change refactor-platform-abstraction. Update Purpose after archive.
## Requirements
### Requirement: Capture Backend Trait

The system SHALL define a `CaptureBackend` trait that abstracts platform-specific capture operations, enabling consistent capture behavior across Windows, Linux, and macOS.

#### Scenario: Start window capture via trait

- **GIVEN** a platform-specific backend implementing `CaptureBackend`
- **WHEN** `start_window_capture` is called with a valid window ID
- **THEN** the backend returns a frame receiver and stop handle
- **AND** frames are delivered through the receiver until stopped

#### Scenario: Start region capture via trait

- **GIVEN** a platform-specific backend implementing `CaptureBackend`
- **WHEN** `start_region_capture` is called with a valid region specification
- **THEN** the backend returns a frame receiver and stop handle
- **AND** frames contain only the specified region cropped from the source

#### Scenario: Start display capture via trait

- **GIVEN** a platform-specific backend implementing `CaptureBackend`
- **WHEN** `start_display_capture` is called with a valid display ID
- **THEN** the backend returns a frame receiver and stop handle
- **AND** frames capture the entire display content

#### Scenario: Capture error handling

- **GIVEN** a platform-specific backend implementing `CaptureBackend`
- **WHEN** capture cannot be started (invalid target, permissions, etc.)
- **THEN** the backend returns a `CaptureError` with descriptive message

### Requirement: Window Enumeration Trait

The system SHALL define a `WindowEnumerator` trait that abstracts platform-specific window listing, returning a consistent `WindowInfo` structure across all platforms.

#### Scenario: List available windows

- **GIVEN** a platform-specific backend implementing `WindowEnumerator`
- **WHEN** `list_windows` is called
- **THEN** a list of `WindowInfo` structs is returned
- **AND** each struct contains window ID, title, and process name

#### Scenario: Filter non-capturable windows

- **GIVEN** a platform-specific backend implementing `WindowEnumerator`
- **WHEN** `list_windows` is called
- **THEN** system windows, invisible windows, and tool windows are excluded
- **AND** only user-facing application windows are returned

### Requirement: Monitor Enumeration Trait

The system SHALL define a `MonitorEnumerator` trait that abstracts platform-specific monitor listing, returning a consistent `MonitorInfo` structure across all platforms.

#### Scenario: List available monitors

- **GIVEN** a platform-specific backend implementing `MonitorEnumerator`
- **WHEN** `list_monitors` is called
- **THEN** a list of `MonitorInfo` structs is returned
- **AND** each struct contains ID, name, position, dimensions, and primary flag

#### Scenario: Primary monitor ordering

- **GIVEN** a platform-specific backend implementing `MonitorEnumerator`
- **WHEN** `list_monitors` is called on a multi-monitor system
- **THEN** the primary monitor appears first in the returned list

#### Scenario: Windows DPI scale factor detection

- **GIVEN** the application is running on Windows
- **WHEN** `list_monitors` is called
- **THEN** each `MonitorInfo.scale_factor` reflects the actual Windows DPI scaling
- **AND** a 100% scaled monitor returns `scale_factor: 1.0`
- **AND** a 125% scaled monitor returns `scale_factor: 1.25`
- **AND** a 150% scaled monitor returns `scale_factor: 1.5`
- **AND** a 200% scaled monitor returns `scale_factor: 2.0`

### Requirement: Highlight Provider Trait

The system SHALL define a `HighlightProvider` trait that abstracts platform-specific visual highlight rendering for selection feedback.

#### Scenario: Show display highlight

- **GIVEN** a platform-specific backend implementing `HighlightProvider`
- **WHEN** `show_highlight` is called with position and dimensions
- **THEN** a visible border is rendered at the specified location
- **AND** the highlight is non-interactive (click-through)
- **AND** the highlight auto-dismisses after a brief animation

### Requirement: Platform Module Organization

The system SHALL organize platform-specific code into dedicated subdirectories under `capture/`, with compile-time selection based on target OS.

#### Scenario: Windows platform module

- **GIVEN** the application is compiled for Windows
- **WHEN** the capture module is loaded
- **THEN** the `capture/windows/` module is compiled and used
- **AND** Windows-specific dependencies are included

#### Scenario: Linux platform module

- **GIVEN** the application is compiled for Linux
- **WHEN** the capture module is loaded
- **THEN** the `capture/linux/` module is compiled and used
- **AND** Linux-specific dependencies are included

#### Scenario: macOS platform module

- **GIVEN** the application is compiled for macOS
- **WHEN** the capture module is loaded
- **THEN** the `capture/macos/` module is compiled and used
- **AND** macOS-specific dependencies are included

### Requirement: Stub Implementations for Unsupported Platforms

The system SHALL provide stub implementations for platforms without full capture support, returning clear "not implemented" errors.

#### Scenario: Capture on unimplemented platform

- **GIVEN** the application is running on a platform without capture implementation
- **WHEN** any capture operation is attempted
- **THEN** a `CaptureError::NotImplemented` error is returned
- **AND** the error message indicates the platform is not yet supported

#### Scenario: Enumeration on unimplemented platform

- **GIVEN** the application is running on a platform without enumeration implementation
- **WHEN** window or monitor enumeration is attempted
- **THEN** an empty list is returned or appropriate error

### Requirement: Shared Type Definitions

The system SHALL define platform-agnostic types for cross-platform data exchange, ensuring consistent API regardless of underlying platform.

#### Scenario: WindowInfo consistency

- **GIVEN** any platform-specific `WindowEnumerator` implementation
- **WHEN** `list_windows` returns results
- **THEN** all results conform to the shared `WindowInfo` struct
- **AND** the struct is serializable for frontend consumption

#### Scenario: MonitorInfo consistency

- **GIVEN** any platform-specific `MonitorEnumerator` implementation
- **WHEN** `list_monitors` returns results
- **THEN** all results conform to the shared `MonitorInfo` struct
- **AND** the struct is serializable for frontend consumption

#### Scenario: CapturedFrame consistency

- **GIVEN** any platform-specific `CaptureBackend` implementation
- **WHEN** frames are captured
- **THEN** all frames conform to the shared `CapturedFrame` struct
- **AND** pixel data is in BGRA format regardless of platform

### Requirement: Platform-Conditional Dependencies

The system SHALL configure Cargo.toml with platform-conditional dependencies, compiling only the dependencies required for the target platform.

#### Scenario: Windows dependencies

- **GIVEN** the project is compiled for Windows
- **WHEN** dependencies are resolved
- **THEN** `windows-capture` and `windows` crates are included
- **AND** Linux/macOS-specific crates are excluded

#### Scenario: Cross-platform compilation

- **GIVEN** the project is compiled for any supported platform
- **WHEN** dependencies are resolved
- **THEN** only dependencies for the target platform are compiled
- **AND** build succeeds without missing dependency errors

### Requirement: Hyprland Backend Selection

The Linux backend SHALL detect and initialize Hyprland-specific capture when available.

#### Scenario: Backend selection on Wayland/Hyprland

- **WHEN** the application starts on Linux with Hyprland compositor
- **THEN** the `LinuxBackend` detects Hyprland via `HYPRLAND_INSTANCE_SIGNATURE`
- **AND** all capture operations use PipeWire via xdg-desktop-portal
- **AND** window/monitor enumeration uses Hyprland IPC

#### Scenario: Unsupported Linux environment

- **WHEN** the application starts on Linux without Hyprland
- **THEN** an error message is displayed: "This application requires Hyprland compositor"
- **AND** capture functionality is disabled

### Requirement: Window Handle Mapping for Wayland

The system SHALL map Hyprland window addresses to the shared `WindowInfo.handle` field.

#### Scenario: Window info on Wayland/Hyprland

- **WHEN** windows are enumerated on Hyprland
- **THEN** `WindowInfo.handle` contains the Hyprland window address as isize
- **AND** `WindowInfo.title` contains the window title
- **AND** `WindowInfo.process_name` contains the application class

