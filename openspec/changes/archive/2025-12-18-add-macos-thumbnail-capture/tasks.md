## 1. Implementation

- [x] 1.1 Implement display thumbnail capture using `CGDisplayCreateImage`
  - Get CGDirectDisplayID from monitor_id string
  - Capture display to CGImage
  - Convert CGImage BGRA data to JPEG thumbnail using shared utilities
  - Return base64-encoded result

- [x] 1.2 Implement window thumbnail capture using `CGWindowListCreateImage`
  - Use window handle as CGWindowID
  - Capture window bounds using kCGWindowListOptionIncludingWindow
  - Convert CGImage BGRA data to JPEG thumbnail
  - Return base64-encoded result

- [x] 1.3 Implement region preview capture
  - Capture display containing the region
  - Get monitor scale factor for coordinate conversion
  - Convert logical coordinates to physical pixels
  - Crop captured frame to region bounds
  - Convert to JPEG preview using shared utilities

- [x] 1.4 Handle permission requirements
  - Screen recording permission check before capture
  - Return appropriate CaptureError::PermissionDenied if not granted

## 2. Testing

- [x] 2.1 Add unit tests for crop_frame helper function
- [x] 2.2 Add integration tests for display thumbnail capture
- [x] 2.3 Add integration tests for window thumbnail capture
- [x] 2.4 Add integration tests for region preview capture
- [ ] 2.5 Manual testing on macOS with Retina display (2x scale)
- [ ] 2.6 Manual testing on macOS with standard display (1x scale)

## 3. Validation

- [x] 3.1 Run `cargo clippy` on macOS to check for warnings
- [x] 3.2 Run `cargo test` on macOS to verify all tests pass
- [ ] 3.3 Manual end-to-end testing with OmniRec UI
  - Verify window list shows thumbnails
  - Verify display list shows thumbnails
  - Verify region preview updates correctly
