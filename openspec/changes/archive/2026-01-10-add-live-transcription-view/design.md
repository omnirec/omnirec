## Context

OmniRec currently supports voice transcription during recording, producing a markdown transcript file. However, users cannot see transcription output until recording completes. This change adds a live transcription view that displays segments as they are generated.

### Constraints

- The transcription processing happens in `omnirec-service` (separate process), not in the Tauri app
- Current architecture writes transcript segments directly to file; no event emission exists
- Must maintain the existing transcript file output (the window is supplementary, not a replacement)
- Should follow the existing theme system and window styling patterns

## Goals / Non-Goals

**Goals:**
- Display transcription segments in real-time during recording
- Provide a separate, movable/resizable window for the transcript
- Allow users to opt-out via configuration setting
- Match the application's visual theme

**Non-Goals:**
- Editing or correcting transcription in the live view
- Saving window position/size preferences (future enhancement)
- Adding transcript to video as subtitles (separate feature)

## Decisions

### Decision: Event-based architecture for segment delivery

The `TranscriptionQueue` worker thread will be modified to invoke a callback when a segment is successfully transcribed. This callback pattern allows:
- The existing file-writing behavior to remain unchanged
- A new event emission path to be added without modifying core transcription logic
- Future extensions (e.g., sending segments over network) without restructuring

**Alternatives considered:**
1. **Polling transcript file** - Simpler but introduces latency and file system overhead
2. **Shared memory queue** - Complex, unnecessary for the expected throughput
3. **Direct IPC to frontend** - Chosen approach via Tauri events is idiomatic and efficient

### Decision: Separate window vs. embedded panel

Using a separate Tauri window rather than embedding the transcript in the main window because:
- Main window has fixed dimensions by design
- Transcript can be positioned anywhere on screen, including secondary monitors
- User can size the transcript window to their preference
- Recording workflow is not interrupted by transcript visibility

### Decision: Window styling approach

The transcript window will:
- Use `decorations: false` and `transparent: true` like the main window
- Include its own close button in the same style as main window
- Apply theme via CSS classes (`theme-light`/`theme-dark`)
- Use the same border radius and border styling as main window

## Data Flow

```
[omnirec-service]
    TranscriptionQueue worker
        → transcribes segment
        → calls on_segment callback
        → ServiceState emits IPC event

[omnirec-app (Tauri)]
    IPC listener receives segment event
        → emits Tauri event "transcription-segment"

[Frontend - Transcript Window]
    Event listener receives "transcription-segment"
        → appends segment to display
        → auto-scrolls to bottom
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Service IPC adds latency to segment display | Acceptable for UI purposes; transcription already has inherent latency |
| Window management complexity | Follow existing region-selector pattern for window lifecycle |
| Theme sync between windows | Pass theme mode via window URL params or emit theme-change events |

## Open Questions

- Should the transcript window be always-on-top during recording? (Initial implementation: no, let user decide via OS window management)
- Should there be a keyboard shortcut to toggle transcript visibility? (Future enhancement)
