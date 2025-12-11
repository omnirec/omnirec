# Design: Pre-Capture Thumbnail Support

## Context

OmniRec displays windows and displays as text-only lists. Users must identify targets by title alone, which is error-prone when multiple windows have similar names. Adding visual thumbnails improves target identification significantly.

The main technical challenge is capturing thumbnails efficiently without blocking the UI or consuming excessive resources. Each platform has different APIs for capturing single frames.

## Goals

- Display thumbnails for all windows and displays in selection lists
- Show region preview when the region selector is active
- Maintain UI responsiveness (thumbnails must not block main thread)
- Keep memory usage reasonable (small thumbnails, efficient caching)
- Auto-refresh thumbnails periodically (5-second interval)

## Non-Goals

- High-resolution previews (small thumbnails are sufficient)
- Video preview/live streaming of capture targets
- Thumbnail persistence across app restarts
- Windows/macOS implementation (stubbed for now)

## Decisions

### 1. Thumbnail Dimensions

**Decision**: Target 160px width, preserve aspect ratio, max 90px height.

**Rationale**: This size is large enough to identify content but small enough to keep memory low. A typical BGRA thumbnail at 160x90 is ~57KB, so 20 windows would use ~1.1MB—acceptable overhead.

### 2. Capture Strategy

**Decision**: Use a dedicated async Tauri command that captures a single frame, scales it, and returns base64-encoded JPEG.

**Rationale**: 
- Base64 JPEG is easily displayable in `<img>` tags
- JPEG compression reduces payload size (~5-10KB per thumbnail)
- Async command keeps UI responsive
- Single-frame capture avoids the overhead of starting a full recording session

### 3. Linux/Wayland Implementation

**Decision**: Use the existing PipeWire/portal flow with our custom picker.

**Rationale**: 
- The custom picker already auto-approves capture requests
- PipeWire provides efficient frame capture
- Reuses existing infrastructure; no new portal permissions needed
- Capture one frame, immediately stop the stream

**Flow**:
1. Frontend calls `get_window_thumbnail(handle)` or `get_display_thumbnail(id)`
2. Backend sets IPC state for the target
3. Portal request triggers, picker auto-approves
4. PipeWire delivers one frame
5. Backend scales and JPEG-encodes the frame
6. Backend returns base64 string to frontend

### 4. Thumbnail Caching

**Decision**: Cache thumbnails in frontend memory with 5-second TTL.

**Rationale**:
- Avoids redundant capture requests during rapid UI updates
- 5-second refresh matches the auto-refresh interval
- Simple Map-based cache with timestamp comparison
- No persistence needed (thumbnails are ephemeral)

### 5. Region Preview

**Decision**: Capture region preview on selector confirm (mouse release), throttled to max 1 update per second.

**Rationale**:
- Continuous capture during drag would be too resource-intensive
- Preview on release gives feedback without performance impact
- 1-second throttle prevents rapid repeated captures

### 6. Placeholder Handling

**Decision**: Show a generic placeholder icon when thumbnail capture fails.

**Rationale**:
- Some windows may be DRM-protected or minimized
- Graceful degradation keeps UI consistent
- CSS placeholder styling is simpler than per-app icon lookup

### 7. Refresh Strategy

**Decision**: Auto-refresh visible thumbnails every 5 seconds while the selection list is displayed.

**Rationale**:
- Windows change content frequently; stale thumbnails are misleading
- 5 seconds balances freshness vs. resource usage
- Only refresh when list is visible (pause when recording or switching tabs)
- Batch requests to avoid thundering herd

## Architecture

```
Frontend (TypeScript)                    Backend (Rust)
┌─────────────────────┐                 ┌─────────────────────────┐
│ Window/Display List │                 │ Tauri Commands          │
│ ┌─────────────────┐ │  invoke()       │ ┌─────────────────────┐ │
│ │ ThumbnailCache  │─┼────────────────▶│ │ get_window_thumbnail│ │
│ │ (Map + TTL)     │ │                 │ │ get_display_thumbnail│ │
│ └─────────────────┘ │                 │ │ get_region_preview   │ │
│                     │                 │ └──────────┬──────────┘ │
│ Auto-refresh timer  │                 │            │            │
│ (5s interval)       │                 │ ┌──────────▼──────────┐ │
└─────────────────────┘                 │ │ ThumbnailCapture    │ │
                                        │ │ (trait)             │ │
                                        │ └──────────┬──────────┘ │
                                        │            │            │
                        ┌───────────────┼────────────┼────────────┤
                        │               │            │            │
                ┌───────▼─────┐  ┌──────▼─────┐  ┌──────▼─────┐   │
                │ Linux       │  │ Windows    │  │ macOS      │   │
                │ (PipeWire)  │  │ (stub)     │  │ (stub)     │   │
                └─────────────┘  └────────────┘  └────────────┘   │
                                        └─────────────────────────┘
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Portal requests may be slow on first call | Cache aggressively; show placeholder while loading |
| PipeWire stream setup overhead for single frame | Consider reusing stream for batch captures |
| Memory usage with many windows | Fixed thumbnail size limits per-image memory |
| Thumbnails block window list loading | Load thumbnails async after list is displayed |

## Open Questions

1. Should we batch thumbnail requests (capture all windows in one portal session)?
   - Recommendation: Start simple (one request per thumbnail), optimize if needed.

2. Should thumbnails load lazily as items scroll into view?
   - Recommendation: No for v1 (lists are typically short), consider for future.
