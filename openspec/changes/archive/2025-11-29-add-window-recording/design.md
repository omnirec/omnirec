# Design: Basic Window Recording

## Context

This is the first implementation of recording functionality for the Screen Recorder application. The goal is to establish a minimal, working foundation that captures a selected window and saves it as an MP4 file. The implementation targets Windows first, using native high-performance APIs.

**Stakeholders**: End users who need to record application windows for tutorials, bug reports, or content creation.

**Constraints**:
- Windows-only for initial implementation (per user request for native Windows APIs)
- Must use high-performance capture (DXGI, not GDI)
- Output must be a common format (MP4 with H.264)
- No audio capture in this iteration (simplicity first)

## Goals / Non-Goals

**Goals:**
- Enumerate visible windows and display them in a selectable list
- Capture frames from a selected window using DXGI Desktop Duplication
- Encode frames to H.264/MP4 using FFmpeg (via ffmpeg-sidecar)
- Provide simple record/stop toggle button
- Save output to user's Videos folder with timestamp filename

**Non-Goals:**
- Audio capture (future enhancement)
- Region selection (separate capability)
- Full-screen capture (separate capability)
- Recording settings/configuration (future enhancement)
- Pause/resume functionality (future enhancement)
- macOS/Linux support (future platform work)

## Decisions

### Decision 1: Use `windows-capture` crate for frame capture

**What**: Use the `windows-capture` crate which wraps Windows.Graphics.Capture API.

**Why**: 
- High-level Rust API over Windows.Graphics.Capture (WGC)
- WGC is the modern, recommended API for screen/window capture on Windows 10+
- Better performance than DXGI Desktop Duplication for window capture specifically
- Handles GPU-to-CPU frame transfer efficiently
- Active maintenance and good documentation

**Alternatives considered**:
- Raw DXGI Desktop Duplication: Lower-level, more complex, better for full-screen but WGC is preferred for window capture
- `scrap` crate: Cross-platform but lower performance on Windows
- `win-screenshot`: Screenshot-only, not suitable for video

### Decision 2: Use `ffmpeg-sidecar` for encoding

**What**: Bundle FFmpeg as a sidecar binary and communicate via stdin/stdout.

**Why**:
- Simplest integration path—no complex FFI bindings
- FFmpeg handles all encoding complexity (H.264, container, etc.)
- Easy to update FFmpeg version independently
- Well-tested in production applications
- Supports piping raw frames directly to FFmpeg process

**Alternatives considered**:
- Native FFmpeg bindings (`ffmpeg-next`): Complex build, linking issues on Windows
- `ez-ffmpeg`: Newer, less battle-tested
- Custom encoding with `x264` bindings: Too low-level for MVP

### Decision 3: Simple state machine for recording control

**What**: Three-state model: Idle → Recording → Saving

**Why**:
- Minimal complexity for MVP
- Clear user feedback about current state
- Easy to extend later with pause/resume

**State transitions**:
```
Idle --[start_recording]--> Recording --[stop_recording]--> Saving --[save_complete]--> Idle
                                ^                              |
                                |______[save_failed]___________|
```

### Decision 4: Frame pipeline architecture

**What**: Capture thread → Channel → Encoder thread

**Why**:
- Decouples capture rate from encode rate
- Prevents frame drops if encoding is temporarily slow
- Allows capture at consistent intervals

**Data flow**:
```
[WGC Capture] --> [BGRA Frame Buffer] --> [Channel] --> [FFmpeg stdin]
     ^                                         |
     |                                         v
  60 FPS                              H.264 encoding
```

## Risks / Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| FFmpeg sidecar increases bundle size (~80MB) | Larger download | Accept for MVP; consider shared FFmpeg later |
| Windows.Graphics.Capture requires Win10 1903+ | Older Windows unsupported | Document minimum requirements; acceptable |
| Frame channel backpressure if encoding too slow | Memory growth, eventual OOM | Bounded channel with frame drop policy |
| Window may close during recording | Recording fails | Detect and gracefully stop recording |

## Migration Plan

Not applicable—this is new functionality with no existing behavior to migrate.

## Open Questions

1. **Default frame rate**: Should we default to 30 FPS or 60 FPS? 
   - *Recommendation*: 30 FPS for smaller files and adequate quality

2. **Output location**: User's Videos folder, or prompt for location?
   - *Recommendation*: Videos folder with auto-generated name for MVP; add "Save As" later

3. **FFmpeg bundling**: Bundle full FFmpeg or minimal build?
   - *Recommendation*: Use ffmpeg-sidecar's default which includes common codecs
