## ADDED Requirements

### Requirement: Windows Thumbnail Implementation

The system SHALL implement thumbnail capture on Windows using the Windows.Graphics.Capture API via the `windows-capture` crate.

#### Scenario: Display thumbnail on Windows

- **WHEN** `capture_display_thumbnail` is called on Windows with a valid monitor ID
- **THEN** the backend captures a single frame from the monitor using Windows.Graphics.Capture
- **AND** the frame is scaled to max 320px width (preserving aspect ratio, max 180px height)
- **AND** the frame is encoded as JPEG
- **AND** the result is returned as a base64-encoded string
- **AND** capture completes within 200ms typical latency

#### Scenario: Window thumbnail on Windows

- **WHEN** `capture_window_thumbnail` is called on Windows with a valid window handle
- **THEN** the backend captures a single frame from the window using Windows.Graphics.Capture
- **AND** the frame is scaled to max 320px width (preserving aspect ratio, max 180px height)
- **AND** the frame is encoded as JPEG
- **AND** the result is returned as a base64-encoded string

#### Scenario: Region preview on Windows

- **WHEN** `capture_region_preview` is called on Windows with monitor ID and region coordinates
- **THEN** the backend captures a single frame from the monitor
- **AND** the frame is cropped to the region bounds (accounting for DPI scale factor)
- **AND** the cropped frame is scaled to fit preview dimensions (max 400px width, 300px height)
- **AND** the frame is encoded as JPEG
- **AND** the result is returned as a base64-encoded string

#### Scenario: Windows thumbnail capture handles DPI scaling

- **GIVEN** a monitor with DPI scale factor greater than 1.0
- **WHEN** `capture_region_preview` is called with logical pixel coordinates
- **THEN** the coordinates are converted to physical pixels for cropping
- **AND** the resulting preview correctly shows the specified region

#### Scenario: Windows thumbnail capture fails gracefully

- **WHEN** thumbnail capture fails on Windows (window closed, permissions, DRM, etc.)
- **THEN** the backend returns an appropriate `CaptureError`
- **AND** the frontend displays a placeholder image
- **AND** no crash or hang occurs

## REMOVED Requirements

### Requirement: Windows/macOS Thumbnail Stubs

**Reason**: Windows is now fully implemented; macOS stub requirement should be split out.

**Migration**: The Windows stub behavior is replaced by the Windows Thumbnail Implementation requirement above. A separate macOS-only stub requirement should be added if needed.

## ADDED Requirements

### Requirement: macOS Thumbnail Stub

The system SHALL provide a stub implementation for macOS that returns "not implemented" errors until full implementation is completed.

#### Scenario: Thumbnail on macOS (stub)

- **WHEN** any thumbnail capture is requested on macOS
- **THEN** the backend returns `Err(CaptureError::NotImplemented)`
- **AND** the frontend displays a placeholder for all items
