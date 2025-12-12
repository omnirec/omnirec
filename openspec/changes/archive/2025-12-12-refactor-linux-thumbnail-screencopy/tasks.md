## 1. Wayland Infrastructure

- [x] 1.1 Extract/refactor shared Wayland connection from highlight module
- [x] 1.2 Add screencopy protocol bindings via wayland-protocols-wlr
- [x] 1.3 Implement output lookup by name (map monitor ID to wl_output)

## 2. Screencopy Module Implementation

- [x] 2.1 Create `src-tauri/src/capture/linux/screencopy.rs` module
- [x] 2.2 Implement `capture_output()` function with buffer handling
- [x] 2.3 Handle `buffer`, `ready`, and `failed` frame events
- [x] 2.4 Implement SHM buffer allocation for frame data
- [x] 2.5 Return captured frame as BGRA byte array

## 3. Display Thumbnail Integration

- [x] 3.1 Rewrite `capture_display_thumbnail` to use screencopy
- [x] 3.2 Convert screencopy output to JPEG thumbnail
- [x] 3.3 Return `NotSupported` error when screencopy unavailable
- [x] 3.4 Test display thumbnail capture latency improvement

## 4. Window Thumbnail with Crop

- [x] 4.1 Query Hyprland IPC for window geometry in `capture_window_thumbnail`
- [x] 4.2 Determine output containing the window
- [x] 4.3 Capture output via screencopy
- [x] 4.4 Crop frame to window bounds before encoding
- [x] 4.5 Test window thumbnail capture with various window positions

## 5. Region Preview with Crop

- [x] 5.1 Rewrite `capture_region_preview` to use screencopy
- [x] 5.2 Capture monitor, crop to region bounds
- [x] 5.3 Test region preview capture latency

## 6. Error Handling

- [x] 6.1 Handle screencopy failures (busy compositor, DRM content)
- [x] 6.2 Return appropriate errors for frontend placeholder display
- [x] 6.3 Remove unused portal-based thumbnail code paths

## 7. Testing and Validation

- [x] 7.1 Test on Hyprland (wlroots compositor)
- [x] 7.2 Measure latency improvement (target: <50ms vs 200-600ms)
- [x] 7.3 Test thumbnail auto-refresh with screencopy (every 5s)
- [x] 7.4 Verify no regressions in recording flow (still uses portal/PipeWire)
