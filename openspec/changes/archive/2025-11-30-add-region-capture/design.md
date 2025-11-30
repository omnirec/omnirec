# Design: Region Capture

## Context

The screen recorder currently supports window capture using Windows.Graphics.Capture API via the `windows-capture` crate. Users have requested the ability to capture arbitrary rectangular regions of the screen, which requires a different capture approach and a new UI for region selection.

**Stakeholders:**
- End users who need flexible screen capture options
- Developers maintaining cross-platform compatibility

**Constraints:**
- Must work with existing FFmpeg encoding pipeline
- Must support multiple monitors
- Performance must remain acceptable (minimal CPU/GPU overhead)
- Must integrate cleanly with existing UI patterns

## Goals / Non-Goals

**Goals:**
- Enable capture of user-defined screen regions
- Support multi-monitor setups for region selection
- Provide intuitive drag-to-select and resize UI
- Maintain existing window capture functionality

**Non-Goals:**
- Cross-monitor region capture (spans multiple displays) - too complex for initial implementation
- Follow-cursor capture mode
- Real-time region adjustment during recording
- macOS/Linux implementation (Windows-first, platform abstraction for later)

## Decisions

### 1. Region Selection Overlay Window

**Decision:** Create a separate transparent borderless Tauri window for region selection.

**Alternatives considered:**
- **System-level overlay using Win32 API**: More complex, requires additional unsafe code, harder to style
- **Capture full screen and crop in encoder**: Wastes resources capturing unwanted areas
- **Use existing window with absolute positioning**: Doesn't work well across monitors

**Rationale:** A Tauri overlay window provides:
- Easy styling with CSS
- Proper multi-monitor support via Tauri's monitor APIs
- Clean separation of concerns
- Consistent behavior across platforms (future-proofing)

### 2. Display Enumeration

**Decision:** Use Tauri's built-in monitor APIs (`available_monitors()`) for display enumeration.

**Rationale:** 
- Already available, no additional dependencies
- Returns virtual screen coordinates needed for region positioning
- Works cross-platform

### 3. Capture Implementation

**Decision:** Use Windows.Graphics.Capture API with monitor target instead of window target.

**Alternatives considered:**
- **DXGI Desktop Duplication**: Lower-level, more complex, but potentially better performance
- **GDI BitBlt**: Legacy, works everywhere but slow
- **windows-capture crate with Monitor**: Existing dependency, consistent API

**Rationale:** The `windows-capture` crate already supports `Monitor` targets. Using the same capture mechanism for both window and region capture minimizes code changes and maintains consistency.

### 4. Region Cropping Strategy

**Decision:** Capture the entire monitor containing the region, then crop frames in the encoding pipeline.

**Alternatives considered:**
- **Hardware crop at capture level**: Not supported by Windows.Graphics.Capture
- **Crop before FFmpeg**: Additional memory copy, but simpler pipeline

**Rationale:** Cropping in the frame processing step before sending to FFmpeg:
- Avoids encoding unnecessary pixels
- Reduces memory bandwidth to encoder
- Simple implementation using buffer slicing

### 5. Selection UI Interaction

**Decision:** Click-drag to create selection, then resize/reposition with handles and drag.

**Behavior:**
1. User clicks "Select Region" button
2. Overlay window appears covering all monitors with semi-transparent background
3. User clicks and drags to create initial selection rectangle
4. Selection shows resize handles at corners and edges
5. User can drag handles to resize or drag center to reposition
6. "Confirm" button finalizes selection, overlay closes
7. User can then click "Record" to start capture

**Selection rectangle features:**
- Minimum size: 100x100 pixels
- Visual feedback: highlighted border, dimension display
- Snap-to-edge optional enhancement (future)

## Risks / Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| Monitor capture has higher overhead than window capture | Performance degradation | Crop early in pipeline; optimize frame copy |
| Overlay window may not appear above all windows | Poor UX | Set always-on-top flag; test with fullscreen apps |
| Multi-monitor DPI differences | Incorrect region coordinates | Use virtual screen coordinates; account for DPI scaling |
| User confusion between modes | UX friction | Clear mode toggle UI; visual distinction between window/region selection |

## Component Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                       Frontend (TypeScript)                  │
├─────────────────────────────────────────────────────────────┤
│  Main Window              │  Selection Overlay Window       │
│  ├─ Mode Toggle           │  ├─ Canvas/HTML overlay         │
│  ├─ Window List (mode=win)│  ├─ Drag selection handler      │
│  ├─ Region Display        │  ├─ Resize handles              │
│  │   (mode=region)        │  └─ Confirm/Cancel buttons      │
│  └─ Record Button         │                                 │
└───────────────┬───────────┴──────────────┬──────────────────┘
                │                          │
                │ Tauri Commands           │ Window Events
                ▼                          ▼
┌─────────────────────────────────────────────────────────────┐
│                       Backend (Rust)                         │
├─────────────────────────────────────────────────────────────┤
│  lib.rs                                                      │
│  ├─ get_monitors() -> Vec<MonitorInfo>                      │
│  ├─ start_region_recording(monitor_id, region) -> ()        │
│  └─ existing commands...                                     │
├─────────────────────────────────────────────────────────────┤
│  capture/                                                    │
│  ├─ mod.rs                                                   │
│  ├─ windows_list.rs (existing)                              │
│  ├─ monitor_list.rs (new)                                   │
│  ├─ recorder.rs (modified - add monitor capture)            │
│  └─ region_recorder.rs (new - capture + crop)               │
├─────────────────────────────────────────────────────────────┤
│  state.rs                                                    │
│  └─ RecordingManager (modified - support CaptureTarget enum)│
└─────────────────────────────────────────────────────────────┘
```

## API Design

### New Types

```rust
// Monitor information for UI
pub struct MonitorInfo {
    pub id: String,           // Unique identifier
    pub name: String,         // Display name
    pub x: i32,               // Virtual screen X
    pub y: i32,               // Virtual screen Y
    pub width: u32,           // Width in pixels
    pub height: u32,          // Height in pixels
    pub scale_factor: f64,    // DPI scale
    pub is_primary: bool,
}

// Region specification
pub struct CaptureRegion {
    pub monitor_id: String,   // Which monitor
    pub x: i32,               // Region X (relative to monitor)
    pub y: i32,               // Region Y (relative to monitor)
    pub width: u32,           // Region width
    pub height: u32,          // Region height
}

// Capture target enum
pub enum CaptureTarget {
    Window(isize),            // Window handle
    Region(CaptureRegion),    // Screen region
}
```

### New Tauri Commands

```rust
#[tauri::command]
fn get_monitors() -> Vec<MonitorInfo>

#[tauri::command]
async fn start_region_recording(
    monitor_id: String,
    x: i32,
    y: i32, 
    width: u32,
    height: u32,
    state: State<'_, AppState>,
) -> Result<(), String>
```

## Open Questions

1. **Should region selection persist across app restarts?** 
   - Recommendation: No, start fresh each session (simpler)

2. **Should we show a preview of the selected region before recording?**
   - Recommendation: Show static bounds indicator, not live preview (performance)

3. **How to handle if selected monitor is disconnected?**
   - Recommendation: Clear selection, prompt user to reselect
