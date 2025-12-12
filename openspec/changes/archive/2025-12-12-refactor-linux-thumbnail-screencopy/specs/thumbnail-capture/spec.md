## MODIFIED Requirements

### Requirement: Linux Thumbnail Implementation

The system SHALL implement thumbnail capture on Linux/Wayland using the wlr-screencopy protocol. Non-wlroots compositors will display placeholder thumbnails.

#### Scenario: Display thumbnail on Linux/wlroots

- **WHEN** `capture_display_thumbnail` is called on Linux/Hyprland
- **THEN** the backend uses wlr-screencopy to capture the output directly
- **AND** a single frame is captured without portal or PipeWire overhead
- **AND** the frame is scaled and encoded as JPEG thumbnail
- **AND** capture completes within 50ms typical latency

#### Scenario: Window thumbnail on Linux/wlroots

- **WHEN** `capture_window_thumbnail` is called on Linux/Hyprland
- **THEN** the backend queries Hyprland IPC for window geometry
- **AND** the backend captures the output containing the window via screencopy
- **AND** the frame is cropped to the window bounds
- **AND** the cropped frame is scaled and encoded as JPEG thumbnail

#### Scenario: Region preview on Linux/wlroots

- **WHEN** `capture_region_preview` is called on Linux/Hyprland
- **THEN** the backend captures the monitor containing the region via screencopy
- **AND** the frame is cropped to the region bounds
- **AND** the cropped frame is scaled for preview display

#### Scenario: Screencopy unavailable shows placeholder

- **WHEN** any thumbnail capture is requested on Linux
- **AND** the `zwlr_screencopy_manager_v1` protocol is NOT available
- **THEN** the backend returns `Err(CaptureError::NotSupported)`
- **AND** the frontend displays a placeholder image

#### Scenario: Screencopy capture fails gracefully

- **WHEN** wlr-screencopy capture fails (compositor busy, DRM protected content, etc.)
- **THEN** the backend returns an appropriate error
- **AND** the frontend displays a placeholder image
- **AND** no crash or hang occurs

## ADDED Requirements

### Requirement: Screencopy Protocol Support

The system SHALL support the `wlr-screencopy-unstable-v1` Wayland protocol for efficient single-frame capture on wlroots compositors.

#### Scenario: Capture output via screencopy

- **WHEN** `screencopy_capture_output` is called with a valid output name
- **THEN** the backend binds to `zwlr_screencopy_manager_v1` global
- **AND** calls `capture_output` with the target wl_output
- **AND** handles `buffer`, `ready`, and `failed` events
- **AND** returns the captured frame data in BGRA format

#### Scenario: Screencopy buffer allocation

- **WHEN** the compositor sends a `buffer` event
- **THEN** the backend allocates a wl_shm buffer with the specified format and dimensions
- **AND** attaches the buffer to the frame
- **AND** signals ready for capture via `copy` request

#### Scenario: Screencopy includes cursor

- **WHEN** capturing via screencopy
- **THEN** the overlay_cursor parameter is set to 1 (include cursor)
- **AND** the captured frame includes the cursor if visible in the output
