## 1. Backend: Thumbnail Trait and Types

- [x] 1.1 Add `ThumbnailCapture` trait to `capture/mod.rs` with methods: `capture_window_thumbnail`, `capture_display_thumbnail`, `capture_region_preview`
- [x] 1.2 Add thumbnail result types (base64 string wrapper, dimensions)
- [x] 1.3 Add image scaling utility function (BGRA to scaled JPEG base64)
- [x] 1.4 Add `image` and `base64` crates to Cargo.toml dependencies

## 2. Backend: Linux Implementation

- [x] 2.1 Create `capture/linux/thumbnail.rs` with `LinuxThumbnailCapture` struct
- [x] 2.2 Implement `capture_display_thumbnail` using PipeWire single-frame capture
- [x] 2.3 Implement `capture_window_thumbnail` using PipeWire single-frame capture
- [x] 2.4 Implement `capture_region_preview` with frame cropping
- [x] 2.5 Add single-frame PipeWire capture helper (start stream, get one frame, stop)
- [x] 2.6 Wire up IPC state for thumbnail capture targets

## 3. Backend: Windows/macOS Stubs

- [x] 3.1 Create `capture/windows/thumbnail.rs` with stub returning `NotImplemented`
- [x] 3.2 Create `capture/macos/thumbnail.rs` with stub returning `NotImplemented`
- [x] 3.3 Export thumbnail modules from platform mod.rs files

## 4. Backend: Tauri Commands

- [x] 4.1 Add `get_window_thumbnail` command (handle -> base64 or null)
- [x] 4.2 Add `get_display_thumbnail` command (monitor_id -> base64 or null)
- [x] 4.3 Add `get_region_preview` command (monitor_id, x, y, w, h -> base64 or null)
- [x] 4.4 Register commands in `lib.rs`

## 5. Frontend: Thumbnail Cache

- [x] 5.1 Create `ThumbnailCache` (Map-based) with TTL-based expiration (5 seconds)
- [x] 5.2 Add `getCachedThumbnail(key)` and `setCachedThumbnail(key, value)` functions
- [x] 5.3 Implemented cache expiration check in getter

## 6. Frontend: Window List Thumbnails

- [x] 6.1 Update `createWindowItem` to include thumbnail `<img>` element with placeholder
- [x] 6.2 Add `loadWindowThumbnail(handle, imgElement)` async function
- [x] 6.3 Update `loadWindows` to trigger thumbnail loading after list render
- [x] 6.4 Add auto-refresh timer (5s interval) for window thumbnails
- [x] 6.5 Pause/resume refresh based on visibility and recording state

## 7. Frontend: Display List Thumbnails

- [x] 7.1 Update `createDisplayItem` to include thumbnail `<img>` element with placeholder
- [x] 7.2 Add `loadDisplayThumbnail(monitorId, imgElement)` async function
- [x] 7.3 Update `loadDisplays` to trigger thumbnail loading after list render
- [x] 7.4 Add auto-refresh timer (5s interval) for display thumbnails
- [x] 7.5 Pause/resume refresh based on visibility and recording state

## 8. Frontend: Region Preview

- [x] 8.1 Add preview image to region display area via updateRegionDisplay
- [x] 8.2 Add `loadRegionPreview()` async function
- [x] 8.3 Add `loadRegionPreviewThrottled()` with 1-second throttle
- [x] 8.4 Update `updateRegionDisplay` to show preview image when available
- [x] 8.5 Clear preview when region is cancelled or mode changes

## 9. Frontend: Styling

- [x] 9.1 Add CSS for thumbnail images in window/display items (flexbox layout)
- [x] 9.2 Add CSS for placeholder state (gradient background, icon placeholder)
- [x] 9.3 Add CSS for region preview in content area
- [x] 9.4 Ensure thumbnails don't cause layout shift on load (opacity transition)

## 10. Testing and Polish

- [x] 10.1 Verify build succeeds with `cargo build`
- [x] 10.2 Run `cargo clippy` and fix critical warnings
- [ ] 10.3 Test thumbnail loading on Linux/Hyprland (requires manual testing)
- [ ] 10.4 Verify placeholder appears on Windows/macOS (requires manual testing)
- [ ] 10.5 Verify auto-refresh pauses during recording (requires manual testing)
- [ ] 10.6 Verify region preview throttling works correctly (requires manual testing)
- [ ] 10.7 Test with many windows (10+) to verify performance (requires manual testing)
