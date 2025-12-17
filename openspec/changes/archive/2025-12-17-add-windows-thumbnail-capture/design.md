## Context

The Linux thumbnail capture uses wlr-screencopy protocol for fast single-frame capture. Windows needs a different approach using Windows.Graphics.Capture API via the `windows-capture` crate. The key challenge is that `windows-capture` is designed for continuous capture, but we need single-frame snapshots for thumbnails.

**Constraints:**
- Must use existing dependencies (no new crates)
- Thumbnails must capture within ~100ms to feel responsive
- Must handle DPI scaling correctly (already implemented in `monitor_list.rs`)
- Must integrate with existing `ThumbnailCapture` trait

## Goals / Non-Goals

**Goals:**
- Implement `capture_display_thumbnail` for full-display thumbnails
- Implement `capture_window_thumbnail` for window-specific thumbnails
- Implement `capture_region_preview` for region selection preview
- Match Linux behavior: scale to max 320x180 (thumbnails) or 400x300 (previews)
- Handle DPI scaling transparently

**Non-Goals:**
- Optimizing for multiple rapid-fire captures (caching is frontend concern)
- GPU-based scaling (CPU scaling via `image` crate is sufficient)
- Alternative capture APIs (PrintWindow, DWM Thumbnail API)

## Decisions

### Decision 1: Use `windows-capture` with Single-Frame Pattern

Use the existing `windows-capture` crate but with a "capture-and-stop" pattern:
1. Start capture on monitor/window
2. Wait for first frame callback
3. Immediately stop capture
4. Process and return the frame

**Rationale:** Reuses existing, tested infrastructure from region/window capture. The `windows-capture` crate handles all Windows.Graphics.Capture setup complexity.

**Alternatives considered:**
- `PrintWindow` API: Doesn't capture DX/Vulkan content, often shows black for games
- BitBlt: Same limitations, also fails for layered windows
- DWM Thumbnail API: Complex setup, better for live thumbnails than snapshots

### Decision 2: Capture Pattern

**Display thumbnails:** Capture the monitor directly using `Monitor::from_device_name()`.

**Window thumbnails:** Capture the window directly using `Window::from_raw_hwnd()`. This handles minimized windows and DWM composition automatically.

**Region preview:** Capture the monitor, then crop to the specified region (same as region recording but single-frame).

### Decision 3: Synchronous-Style API with Internal Threading

The `ThumbnailCapture` trait expects synchronous returns. The implementation will:
1. Spawn capture thread
2. Use a oneshot channel to receive the first frame
3. Block the calling thread until frame arrives (with timeout)
4. Return the processed thumbnail

This matches how the Linux implementation works with wlr-screencopy (blocking until frame ready).

## Risks / Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| First-frame latency | Medium | Accept 50-100ms typical latency; UI shows placeholder |
| Window capture fails for some apps | Low | Return error, show placeholder (graceful degradation) |
| DPI coordinate mismatch | Medium | Use `monitor_list.rs` scale factor; convert logical->physical |
| Thread resource usage | Low | Threads are short-lived (~100ms); Windows threadpool handles this well |

## Implementation Notes

### Frame Data Flow

```
Monitor/Window -> Windows.Graphics.Capture -> BGRA buffer
                                                |
                                                v
                            crop_frame (if region/window) 
                                                |
                                                v
                              bgra_to_jpeg_thumbnail
                                                |
                                                v
                                    ThumbnailResult (base64)
```

### Key Code Locations

- Stub to replace: `src-tauri/src/capture/windows/thumbnail.rs`
- Shared thumbnail encoding: `src-tauri/src/capture/thumbnail.rs`
- Monitor info (for scale factor): `src-tauri/src/capture/windows/monitor_list.rs`
- Reference patterns: `src-tauri/src/capture/windows/recorder.rs`, `region.rs`

### DPI Handling

Region coordinates from frontend are in logical pixels. The capture returns physical pixels. Scale conversion:
```rust
let physical_x = (logical_x as f64 * scale_factor).round() as i32;
```

This is already implemented in `region.rs` and can be reused.

## Open Questions

None - the approach is straightforward and follows established patterns from Linux implementation and existing Windows capture code.
