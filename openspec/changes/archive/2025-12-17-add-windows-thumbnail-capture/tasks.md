## 1. Core Implementation

- [x] 1.1 Create single-frame capture helper that uses `windows-capture` with immediate stop
- [x] 1.2 Implement `capture_display_thumbnail` using Monitor capture
- [x] 1.3 Implement `capture_window_thumbnail` using Window capture with HWND
- [x] 1.4 Implement `capture_region_preview` using Monitor capture with cropping

## 2. DPI and Coordinate Handling

- [x] 2.1 Add helper to get monitor scale factor by ID (reuse from `monitor_list.rs`)
- [x] 2.2 Ensure region coordinates are converted from logical to physical pixels for cropping

## 3. Error Handling

- [x] 3.1 Add timeout handling (500ms max) to prevent hangs
- [x] 3.2 Map capture errors to appropriate `CaptureError` variants
- [x] 3.3 Handle edge cases: minimized windows, closed windows, invalid monitor IDs

## 4. Testing

- [x] 4.1 Add unit test for display thumbnail capture
- [x] 4.2 Add unit test for window thumbnail capture
- [x] 4.3 Add unit test for region preview with DPI scaling
- [ ] 4.4 Manual test: verify thumbnails display in window/display lists
- [ ] 4.5 Manual test: verify region preview updates correctly

## 5. Validation

- [x] 5.1 Run `cargo clippy` and fix any warnings
- [x] 5.2 Run `cargo test` and ensure all tests pass
- [ ] 5.3 Build and test on Windows with multiple DPI configurations
