## Context

The current Linux thumbnail implementation uses the full Portal + PipeWire flow:
1. IPC selection setup (~50ms)
2. Portal session creation via D-Bus (~100-300ms)
3. Picker subprocess invocation (~50-100ms)
4. PipeWire stream initialization (~50-200ms)
5. Frame capture with variable wait (up to 5s timeout)
6. Stream teardown

This infrastructure is designed for continuous video streaming, not single-frame screenshots. The overhead is acceptable for recording initiation but unacceptable for thumbnail refresh (every 5 seconds, potentially dozens of items).

The `wlr-screencopy-unstable-v1` protocol provides direct framebuffer access optimized for screenshot use cases.

## Goals / Non-Goals

**Goals:**
- Reduce thumbnail capture latency from 200-600ms to 10-50ms
- Improve thumbnail list responsiveness and refresh performance
- Maintain compatibility with current portal-based recording flow
- Keep the implementation simple and maintainable

**Non-Goals:**
- Replace PipeWire for continuous video recording (portal flow remains for that)
- Support non-wlroots compositors (future work; will show placeholders for now)
- Session persistence or caching across captures (each capture is independent)

## Decisions

### Decision: Use wlr-screencopy for all thumbnail capture

**Rationale:** The protocol is purpose-built for single-frame capture with minimal overhead. It directly copies the compositor's framebuffer to shared memory without D-Bus, portal handshakes, or streaming infrastructure.

**Implementation:**
```rust
// Bind to zwlr_screencopy_manager_v1 global
let manager = registry.bind::<ZwlrScreencopyManagerV1>();

// Capture output (includes cursor by default)
let frame = manager.capture_output(1, &output); // overlay_cursor=1

// Handle frame callbacks:
// - buffer: compositor specifies SHM format/dimensions
// - ready: frame data available
// - failed: capture failed
```

### Decision: Implement window thumbnails via output capture + crop

**Rationale:** wlr-screencopy only supports output-level capture, not individual windows. However, we can:
1. Query window geometry via Hyprland IPC (`hyprctl clients -j`)
2. Determine which output contains the window
3. Capture that output
4. Crop to window bounds

This is still faster than portal flow since we avoid all the D-Bus/PipeWire overhead.

**Tradeoff:** Window capture includes any overlapping content. For most use cases (thumbnails), this is acceptable.

### Decision: No portal fallback for thumbnails

**Rationale:** Rather than maintaining two code paths, we will use wlr-screencopy exclusively for thumbnails. Non-wlroots compositors will show placeholder thumbnails until an alternative solution is implemented.

This simplifies the codebase and avoids the complexity of runtime detection and fallback logic. The portal flow remains available for recording, which is the primary use case.

### Decision: Reuse existing wayland-client setup from highlight module

**Rationale:** The highlight module already establishes a Wayland connection and binds to wlr protocols. We can extend this infrastructure for screencopy rather than creating a parallel connection.

The `wayland-protocols-wlr` crate is already a dependency (for layer-shell highlight), so adding screencopy protocol bindings is minimal.

## Alternatives Considered

### Screenshot Portal (`org.freedesktop.portal.Screenshot`)
- Simpler than ScreenCast portal
- Typically requires user confirmation dialog
- Returns file path requiring additional I/O
- **Verdict:** Not suitable for automated thumbnail refresh

### Portal restore tokens for session reuse
- Could skip picker on subsequent captures
- Still initializes new PipeWire stream per capture
- Added complexity for incremental improvement
- **Verdict:** Insufficient benefit; screencopy is dramatically simpler

### Long-running PipeWire session
- One-time setup cost, instant frame extraction thereafter
- Complex lifecycle management (when to create/destroy)
- Memory overhead for idle sessions
- **Verdict:** Better suited for auto-refresh feature; overkill for current scope

### Hyprland screenshot command
- Hyprland has no built-in `hyprctl screenshot` command
- External tools like `grim` use wlr-screencopy internally
- **Verdict:** Not available; use screencopy directly

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     ThumbnailCapture trait                  │
├─────────────────────────────────────────────────────────────┤
│  capture_window_thumbnail()                                 │
│  capture_display_thumbnail()                                │
│  capture_region_preview()                                   │
└────────────────────────────┬────────────────────────────────┘
                             │
                   ┌─────────▼─────────┐
                   │  screencopy.rs    │
                   │  (wlroots only)   │
                   └─────────┬─────────┘
                             │
                   ┌─────────▼─────────────────────────────────┐
                   │  Wayland Connection (shared with highlight)│
                   ├───────────────────────────────────────────┤
                   │  zwlr_screencopy_manager_v1               │
                   │  zwlr_layer_shell_v1 (existing highlight) │
                   └───────────────────────────────────────────┘
```

## Module Structure

```
src-tauri/src/capture/linux/
├── mod.rs                 # Add screencopy module export
├── screencopy.rs          # NEW: wlr-screencopy capture
├── thumbnail.rs           # REWRITE: use screencopy only
├── highlight.rs           # EXISTING: extend wayland setup
├── portal_client.rs       # EXISTING: recording only
└── pipewire_capture.rs    # EXISTING: recording only
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| wlr-screencopy not available on non-wlr compositors | Show placeholder thumbnails; implement alternative later |
| Window crop may include overlapping content | Acceptable for thumbnails |
| Wayland connection complexity | Reuse existing highlight module infrastructure |
| DMA-BUF buffer support varies | Start with SHM buffers; DMA-BUF optional optimization |

## Migration Plan

1. **Phase 1:** Add `screencopy.rs` module with basic output capture
2. **Phase 2:** Rewrite `thumbnail.rs` to use screencopy exclusively
3. **Phase 3:** Add window capture via output crop + Hyprland geometry
4. **Phase 4:** Remove unused portal thumbnail code paths

No user-facing migration required. The change is transparent - thumbnails simply become faster on wlroots compositors.

## Open Questions

1. **Buffer format handling:** Should we prefer DMA-BUF for GPU-accelerated capture, or stick with SHM for simplicity? *Recommendation: Start with SHM; DMA-BUF as future optimization.*

2. **Window decoration handling:** When cropping window thumbnails, should we include shadows/decorations or crop tightly to content? *Recommendation: Use Hyprland's reported window geometry which typically includes decorations.*

3. **Wayland connection lifecycle:** Create connection per-capture or maintain long-lived connection? *Recommendation: Long-lived connection shared with highlight module.*

## References

- [wlr-screencopy-unstable-v1 protocol](https://wayland.app/protocols/wlr-screencopy-unstable-v1)
- [wayland-client crate](https://docs.rs/wayland-client)
- [wayland-protocols-wlr crate](https://docs.rs/wayland-protocols-wlr)
- [Hyprland IPC](https://wiki.hyprland.org/IPC/)
