## ADDED Requirements

### Requirement: macOS Thumbnail Implementation

The system SHALL implement thumbnail capture on macOS using the Core Graphics API (`CGWindowListCreateImage` and `CGDisplayCreateImage`).

#### Scenario: Display thumbnail on macOS

- **WHEN** `capture_display_thumbnail` is called on macOS with a valid display ID
- **THEN** the backend captures a single frame from the display using `CGDisplayCreateImage`
- **AND** the frame is scaled to max 320px width (preserving aspect ratio, max 180px height)
- **AND** the frame is encoded as JPEG
- **AND** the result is returned as a base64-encoded string
- **AND** capture completes within 100ms typical latency

#### Scenario: Window thumbnail on macOS

- **WHEN** `capture_window_thumbnail` is called on macOS with a valid window handle (CGWindowID)
- **THEN** the backend captures a single frame from the window using `CGWindowListCreateImage`
- **AND** the frame is scaled to max 320px width (preserving aspect ratio, max 180px height)
- **AND** the frame is encoded as JPEG
- **AND** the result is returned as a base64-encoded string

#### Scenario: Region preview on macOS

- **WHEN** `capture_region_preview` is called on macOS with display ID and region coordinates
- **THEN** the backend captures a single frame from the display
- **AND** the frame is cropped to the region bounds (accounting for Retina scale factor)
- **AND** the cropped frame is scaled to fit preview dimensions (max 400px width, 300px height)
- **AND** the frame is encoded as JPEG
- **AND** the result is returned as a base64-encoded string

#### Scenario: macOS thumbnail capture handles Retina scaling

- **GIVEN** a Retina display with scale factor greater than 1.0
- **WHEN** `capture_region_preview` is called with logical pixel coordinates
- **THEN** the coordinates are converted to physical pixels for cropping
- **AND** the resulting preview correctly shows the specified region

#### Scenario: macOS thumbnail capture fails gracefully

- **WHEN** thumbnail capture fails on macOS (window closed, permissions, etc.)
- **THEN** the backend returns an appropriate `CaptureError`
- **AND** the frontend displays a placeholder image
- **AND** no crash or hang occurs

#### Scenario: macOS thumbnail capture requires screen recording permission

- **GIVEN** screen recording permission is not granted
- **WHEN** any thumbnail capture is requested on macOS
- **THEN** the backend returns `CaptureError::PermissionDenied`
- **AND** the error message indicates screen recording permission is required

## REMOVED Requirements

### Requirement: macOS Thumbnail Stub

**Reason**: macOS thumbnail capture is now fully implemented using Core Graphics APIs.

**Migration**: The stub behavior is replaced by the macOS Thumbnail Implementation requirement above. All thumbnail operations now return actual captured images instead of `CaptureError::NotImplemented`.
