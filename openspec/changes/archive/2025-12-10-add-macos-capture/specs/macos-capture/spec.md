## ADDED Requirements

### Requirement: macOS Window Enumeration

The system SHALL enumerate all visible, capturable windows on macOS using Core Graphics APIs.

#### Scenario: List available windows on macOS

- **GIVEN** the application is running on macOS
- **WHEN** `list_windows` is called
- **THEN** a list of `WindowInfo` structs is returned for all user-facing windows
- **AND** each struct contains the window ID (CGWindowID), title, and owning application name
- **AND** system windows (menu bar, dock, Notification Center) are excluded

#### Scenario: Filter non-capturable windows on macOS

- **GIVEN** the application is running on macOS
- **WHEN** `list_windows` is called
- **THEN** invisible windows (offscreen, minimized) are excluded
- **AND** windows with empty titles are excluded
- **AND** desktop background windows are excluded

### Requirement: macOS Monitor Enumeration

The system SHALL enumerate all connected displays on macOS using Core Graphics APIs.

#### Scenario: List available monitors on macOS

- **GIVEN** the application is running on macOS
- **WHEN** `list_monitors` is called
- **THEN** a list of `MonitorInfo` structs is returned for all connected displays
- **AND** each struct contains CGDirectDisplayID as string, display name, position, dimensions, and primary flag
- **AND** the primary display is marked with `is_primary: true`

#### Scenario: Handle Retina displays

- **GIVEN** a Retina display is connected
- **WHEN** `list_monitors` is called
- **THEN** the display dimensions reflect the physical pixel resolution
- **AND** the display name indicates if it is Retina (e.g., "Built-in Retina Display")

### Requirement: macOS Window Capture

The system SHALL capture frames from a selected window on macOS using ScreenCaptureKit.

#### Scenario: Start window capture on macOS

- **GIVEN** the application is running on macOS 12.3+
- **AND** screen recording permission is granted
- **WHEN** `start_window_capture` is called with a valid window handle
- **THEN** an SCStream is created with an SCContentFilter targeting the window
- **AND** frames are delivered through the FrameReceiver channel
- **AND** a StopHandle is returned to control capture

#### Scenario: Capture frames at target frame rate

- **GIVEN** window capture is active on macOS
- **WHEN** frames are captured
- **THEN** frames are delivered at approximately 30 FPS (default)
- **AND** frames are in BGRA pixel format
- **AND** frames include the window content without window chrome (client area only)

#### Scenario: Handle window resize during capture

- **GIVEN** window capture is active on macOS
- **WHEN** the target window is resized
- **THEN** capture continues with the new dimensions
- **AND** subsequent frames reflect the new window size

#### Scenario: Handle window close during capture

- **GIVEN** window capture is active on macOS
- **WHEN** the target window is closed
- **THEN** the capture stream ends gracefully
- **AND** the FrameReceiver channel is closed
- **AND** no error is raised (stream simply ends)

### Requirement: macOS Display Capture

The system SHALL capture frames from an entire display on macOS using ScreenCaptureKit.

#### Scenario: Start display capture on macOS

- **GIVEN** the application is running on macOS 12.3+
- **AND** screen recording permission is granted
- **WHEN** `start_display_capture` is called with a valid display ID
- **THEN** an SCStream is created with an SCContentFilter targeting the display
- **AND** frames are delivered through the FrameReceiver channel
- **AND** frames capture the entire display including all visible windows

#### Scenario: Capture cursor in display capture

- **GIVEN** display capture is active on macOS
- **WHEN** frames are captured
- **THEN** the cursor is included in the captured frames
- **AND** the cursor position reflects its actual screen position

### Requirement: macOS Region Capture

The system SHALL capture frames from a user-defined screen region on macOS using ScreenCaptureKit with cropping.

#### Scenario: Start region capture on macOS

- **GIVEN** the application is running on macOS 12.3+
- **AND** screen recording permission is granted
- **WHEN** `start_region_capture` is called with a CaptureRegion
- **THEN** an SCStream is created targeting the specified monitor
- **AND** frames are cropped to the region boundaries
- **AND** the output frame dimensions match the region dimensions

#### Scenario: Region capture includes cursor

- **GIVEN** region capture is active on macOS
- **WHEN** the cursor is within the region
- **THEN** the cursor is included in the captured frames

#### Scenario: Region coordinates are respected

- **GIVEN** region capture is active on macOS
- **WHEN** frames are captured
- **THEN** only pixels within the specified region (x, y, width, height) are included
- **AND** the region coordinates are relative to the monitor origin

### Requirement: macOS Screen Recording Permission

The system SHALL handle macOS screen recording permission requirements gracefully.

#### Scenario: Permission granted

- **GIVEN** the user has granted screen recording permission
- **WHEN** any capture operation is started
- **THEN** capture proceeds normally

#### Scenario: Permission not granted - first request

- **GIVEN** the user has not yet been prompted for screen recording permission
- **WHEN** a capture operation is attempted
- **THEN** macOS displays the permission prompt
- **AND** the capture operation returns `CaptureError::PermissionDenied`
- **AND** the error message instructs the user to grant permission in System Settings

#### Scenario: Permission denied

- **GIVEN** the user has denied screen recording permission
- **WHEN** a capture operation is attempted
- **THEN** the operation returns `CaptureError::PermissionDenied`
- **AND** the error message guides the user to enable permission in System Settings > Privacy & Security > Screen Recording

#### Scenario: Check permission status

- **GIVEN** the application is running on macOS
- **WHEN** the backend is queried for permission status
- **THEN** the current permission state is returned (granted, denied, or undetermined)

### Requirement: macOS Visual Highlight

The system SHALL display a visual highlight border on macOS when previewing capture targets.

#### Scenario: Show window highlight on macOS

- **GIVEN** a window is selected for capture on macOS
- **WHEN** `show_highlight` is called with the window's bounds
- **THEN** a visible border is rendered around the window
- **AND** the highlight is non-interactive (click-through)
- **AND** the highlight auto-dismisses after a brief duration

#### Scenario: Show display highlight on macOS

- **GIVEN** a display is selected for capture on macOS
- **WHEN** `show_highlight` is called with the display's bounds
- **THEN** a visible border is rendered around the display edges
- **AND** the highlight is visible above all other windows

### Requirement: macOS Minimum Version

The system SHALL require macOS 12.3 or later for ScreenCaptureKit-based capture.

#### Scenario: Supported macOS version

- **GIVEN** the application is running on macOS 12.3+
- **WHEN** capture operations are attempted
- **THEN** ScreenCaptureKit APIs are available and functional

#### Scenario: Unsupported macOS version

- **GIVEN** the application is running on macOS prior to 12.3
- **WHEN** capture operations are attempted
- **THEN** a `CaptureError::NotImplemented` error is returned
- **AND** the error message states "macOS 12.3 or later is required for screen recording"

### Requirement: macOS Dependencies

The Cargo.toml SHALL include macOS-specific dependencies conditionally compiled only on macOS targets.

#### Scenario: macOS build includes ScreenCaptureKit crate

- **GIVEN** the project is compiled for macOS
- **WHEN** dependencies are resolved
- **THEN** the `screencapturekit` crate is included
- **AND** the `core-graphics` and `core-foundation` crates are included
- **AND** Windows and Linux specific crates are excluded

#### Scenario: Non-macOS build excludes macOS crates

- **GIVEN** the project is compiled for Windows or Linux
- **WHEN** dependencies are resolved
- **THEN** macOS-specific crates are not compiled
