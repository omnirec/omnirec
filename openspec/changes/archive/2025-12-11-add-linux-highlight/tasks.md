# Tasks: Add Linux Highlight

## 1. Add Dependencies

- [x] 1.1 Add `wayland-client = "0.31"` to Linux dependencies in `Cargo.toml`
- [x] 1.2 Add `wayland-protocols-wlr = { version = "0.3", features = ["client"] }` for layer-shell
- [x] 1.3 Verify build succeeds with new dependencies

## 2. Create Highlight Module

- [x] 2.1 Create `src-tauri/src/capture/linux/highlight.rs`
- [x] 2.2 Add `pub mod highlight;` to `src-tauri/src/capture/linux/mod.rs`
- [x] 2.3 Implement `show_highlight(x: i32, y: i32, width: i32, height: i32)` public function

## 3. Implement Wayland Connection

- [x] 3.1 Implement Wayland display connection in background thread
- [x] 3.2 Implement registry handling to discover globals
- [x] 3.3 Bind to `zwlr_layer_shell_v1` global (with graceful fallback if unavailable)
- [x] 3.4 Bind to `wl_compositor` for surface creation
- [x] 3.5 Bind to `wl_shm` for shared memory buffers

## 4. Implement Layer Surface

- [x] 4.1 Create `wl_surface` via compositor
- [x] 4.2 Create layer surface on `overlay` layer
- [x] 4.3 Configure layer surface properties:
  - Size matching highlight dimensions
  - Anchored to top-left with margin-based positioning
  - Exclusive zone = -1 (no space reservation)
  - Keyboard interactivity = none
- [x] 4.4 Handle layer surface `configure` event
- [x] 4.5 Commit surface after configuration

## 5. Implement Buffer Rendering

- [x] 5.1 Create shared memory pool for buffer
- [x] 5.2 Allocate ARGB8888 buffer of appropriate size
- [x] 5.3 Render border graphic to buffer:
  - Blue (#2196F3) border pixels with full alpha
  - Transparent interior (alpha = 0)
  - 6-8 pixel border width
- [x] 5.4 Attach buffer to surface and commit

## 6. Implement Lifecycle Management

- [x] 6.1 Spawn background thread for highlight display
- [x] 6.2 Run Wayland event loop for 800ms duration
- [x] 6.3 Destroy layer surface and disconnect after timeout
- [x] 6.4 Handle case where new highlight requested while existing one active (cancel previous)

## 7. Update LinuxBackend

- [x] 7.1 Update `HighlightProvider` impl to call `highlight::show_highlight`
- [x] 7.2 Remove stub log message

## 8. Testing

- [x] 8.1 Test highlight for window selection on Hyprland
- [x] 8.2 Test highlight for display selection on Hyprland
- [x] 8.3 Test on multi-monitor setup (verify correct positioning)
- [x] 8.4 Test rapid successive highlights (ensure proper cleanup)
- [x] 8.5 Verify highlight does not capture input (click-through working)
- [ ] 8.6 Test graceful fallback on compositor without layer-shell support
