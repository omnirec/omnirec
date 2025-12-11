## ADDED Requirements

### Requirement: Thumbnail Capture Trait

The system SHALL define a `ThumbnailCapture` trait that abstracts platform-specific single-frame capture for generating thumbnails.

#### Scenario: Capture window thumbnail

- **GIVEN** a platform-specific backend implementing `ThumbnailCapture`
- **WHEN** `capture_window_thumbnail` is called with a valid window handle
- **THEN** the backend captures a single frame from the window
- **AND** the frame is scaled to max 160px width (preserving aspect ratio, max 90px height)
- **AND** the frame is encoded as JPEG
- **AND** the result is returned as a base64-encoded string

#### Scenario: Capture display thumbnail

- **GIVEN** a platform-specific backend implementing `ThumbnailCapture`
- **WHEN** `capture_display_thumbnail` is called with a valid display ID
- **THEN** the backend captures a single frame from the display
- **AND** the frame is scaled to max 160px width (preserving aspect ratio, max 90px height)
- **AND** the frame is encoded as JPEG
- **AND** the result is returned as a base64-encoded string

#### Scenario: Capture region preview

- **GIVEN** a platform-specific backend implementing `ThumbnailCapture`
- **WHEN** `capture_region_preview` is called with monitor ID and region coordinates
- **THEN** the backend captures a single frame from the specified region
- **AND** the frame is scaled to fit the preview area (max 400px width)
- **AND** the frame is encoded as JPEG
- **AND** the result is returned as a base64-encoded string

#### Scenario: Thumbnail capture fails gracefully

- **GIVEN** a platform-specific backend implementing `ThumbnailCapture`
- **WHEN** thumbnail capture fails (target unavailable, permissions, DRM, etc.)
- **THEN** the backend returns `None` or an appropriate error
- **AND** the frontend displays a placeholder image

### Requirement: Window List Thumbnails

The system SHALL display thumbnail images in the window selection list.

#### Scenario: Window list shows thumbnails

- **WHEN** the window list is displayed
- **THEN** each window item includes a thumbnail image area
- **AND** thumbnails are loaded asynchronously after the list is rendered
- **AND** a loading placeholder is shown while thumbnails load

#### Scenario: Thumbnail load failure shows placeholder

- **WHEN** a window thumbnail fails to load
- **THEN** a generic placeholder image is displayed
- **AND** the window title and process name remain visible
- **AND** the item remains selectable

#### Scenario: Window thumbnails auto-refresh

- **WHEN** the window list is visible and the application is idle
- **THEN** thumbnails refresh automatically every 5 seconds
- **AND** refresh pauses during recording
- **AND** refresh pauses when window list is not visible

### Requirement: Display List Thumbnails

The system SHALL display thumbnail images in the display selection list.

#### Scenario: Display list shows thumbnails

- **WHEN** the display list is displayed
- **THEN** each display item includes a thumbnail image area
- **AND** thumbnails are loaded asynchronously after the list is rendered
- **AND** a loading placeholder is shown while thumbnails load

#### Scenario: Display thumbnail load failure shows placeholder

- **WHEN** a display thumbnail fails to load
- **THEN** a generic placeholder image is displayed
- **AND** the display name and resolution remain visible
- **AND** the item remains selectable

#### Scenario: Display thumbnails auto-refresh

- **WHEN** the display list is visible and the application is idle
- **THEN** thumbnails refresh automatically every 5 seconds
- **AND** refresh pauses during recording
- **AND** refresh pauses when display list is not visible

### Requirement: Region Preview

The system SHALL display a preview of the selected region in the region capture tab.

#### Scenario: Region preview on selection confirm

- **WHEN** the user releases the mouse after moving or resizing the region selector
- **THEN** a preview of the selected region is captured
- **AND** the preview is displayed in the region content area
- **AND** preview updates are throttled to max once per second

#### Scenario: Region preview shows selected area

- **WHEN** a region preview is displayed
- **THEN** the preview shows the actual screen content within the selected bounds
- **AND** the preview is scaled to fit the available space (max 400px width)
- **AND** the region dimensions are displayed alongside the preview

#### Scenario: Region preview updates on selector close

- **WHEN** the region selector window is closed (not cancelled)
- **THEN** a final preview is captured and displayed
- **AND** the preview persists until recording starts or region is changed

### Requirement: Thumbnail Performance

The system SHALL capture thumbnails without adversely affecting UI performance or responsiveness.

#### Scenario: Asynchronous thumbnail loading

- **WHEN** thumbnails are requested
- **THEN** capture operations run on background threads
- **AND** the UI remains responsive during capture
- **AND** thumbnail results are delivered via async callbacks

#### Scenario: Thumbnail caching

- **WHEN** a thumbnail is successfully captured
- **THEN** the thumbnail is cached in frontend memory
- **AND** cached thumbnails are used if requested again within 5 seconds
- **AND** cache entries expire and are refreshed after 5 seconds

#### Scenario: Batch thumbnail efficiency

- **WHEN** multiple thumbnails are requested (e.g., on list refresh)
- **THEN** requests are processed without overwhelming the system
- **AND** UI updates progressively as thumbnails become available

### Requirement: Linux Thumbnail Implementation

The system SHALL implement thumbnail capture on Linux/Wayland using PipeWire via the portal flow.

#### Scenario: Window thumbnail on Linux

- **WHEN** `capture_window_thumbnail` is called on Linux/Hyprland
- **THEN** the backend uses the existing IPC/picker mechanism
- **AND** a portal screencast session is created for the window
- **AND** a single PipeWire frame is captured
- **AND** the session is immediately closed after capture

#### Scenario: Display thumbnail on Linux

- **WHEN** `capture_display_thumbnail` is called on Linux/Hyprland
- **THEN** the backend uses the existing IPC/picker mechanism
- **AND** a portal screencast session is created for the display
- **AND** a single PipeWire frame is captured
- **AND** the session is immediately closed after capture

#### Scenario: Region preview on Linux

- **WHEN** `capture_region_preview` is called on Linux/Hyprland
- **THEN** the backend captures the full monitor containing the region
- **AND** the frame is cropped to the region bounds
- **AND** the cropped frame is scaled for preview display

### Requirement: Windows/macOS Thumbnail Stubs

The system SHALL provide stub implementations for Windows and macOS that return "not implemented" errors.

#### Scenario: Thumbnail on Windows (stub)

- **WHEN** any thumbnail capture is requested on Windows
- **THEN** the backend returns `Err(CaptureError::NotImplemented)`
- **AND** the frontend displays a placeholder for all items

#### Scenario: Thumbnail on macOS (stub)

- **WHEN** any thumbnail capture is requested on macOS
- **THEN** the backend returns `Err(CaptureError::NotImplemented)`
- **AND** the frontend displays a placeholder for all items
