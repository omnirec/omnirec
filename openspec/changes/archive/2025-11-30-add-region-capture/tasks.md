# Implementation Tasks

## 1. Backend: Display Enumeration

- [x] 1.1 Create `src-tauri/src/capture/monitor_list.rs` with `MonitorInfo` struct and `list_monitors()` function
- [x] 1.2 Add `get_monitors` Tauri command in `lib.rs`
- [x] 1.3 Write unit tests for monitor enumeration

## 2. Backend: Region Capture

- [x] 2.1 Add `CaptureRegion` struct and `CaptureTarget` enum to capture module
- [x] 2.2 Create `src-tauri/src/capture/region_recorder.rs` with monitor-based capture
- [x] 2.3 Implement frame cropping logic to extract region from full monitor capture
- [x] 2.4 Add `start_region_recording` Tauri command
- [x] 2.5 Modify `RecordingManager` to support `CaptureTarget` enum
- [x] 2.6 Write integration tests for region capture

## 3. Frontend: Capture Mode UI

- [x] 3.1 Add capture mode toggle (Window / Region) to main UI
- [x] 3.2 Create TypeScript types for `MonitorInfo` and `CaptureRegion`
- [x] 3.3 Add "Select Region" button that triggers overlay
- [x] 3.4 Display selected region dimensions when region mode is active
- [x] 3.5 Modify record button logic to check for valid capture target based on mode

## 4. Frontend: Selection Overlay Window

- [x] 4.1 Create new Tauri window configuration for overlay (transparent, frameless, always-on-top)
- [x] 4.2 Create `selection-overlay.html` with canvas for drawing selection
- [x] 4.3 Create `selection-overlay.ts` with selection logic
- [x] 4.4 Implement click-drag to create initial selection rectangle
- [x] 4.5 Add resize handles at corners and edges
- [x] 4.6 Implement center-drag to reposition selection
- [x] 4.7 Add dimension display showing current selection size
- [x] 4.8 Implement minimum size constraint (100x100)
- [x] 4.9 Implement single-monitor constraint
- [x] 4.10 Add Confirm/Cancel buttons with keyboard shortcuts (Enter/Escape)

## 5. Frontend: Overlay Styling

- [x] 5.1 Create `selection-overlay.css` with semi-transparent backdrop
- [x] 5.2 Style selection rectangle with visible border and handles
- [x] 5.3 Add hover/active states for handles
- [x] 5.4 Style dimension display tooltip

## 6. Integration

- [x] 6.1 Wire up overlay window launch from main window
- [x] 6.2 Implement cross-window communication for selection result
- [x] 6.3 Update main window state when region is confirmed
- [x] 6.4 Handle mode switching (preserve region selection when switching away and back)

## 7. Testing & Polish

- [x] 7.1 Test multi-monitor scenarios (different sizes, DPI scales)
- [x] 7.2 Test overlay with fullscreen applications
- [x] 7.3 Test recording quality matches selected region dimensions
- [x] 7.4 Verify recorded video dimensions match region size
- [x] 7.5 Test mode switching during recording is properly disabled
- [x] 7.6 Add error handling for edge cases (monitor disconnect, etc.)

## Dependencies

- Task 2 depends on Task 1 (need monitor info for region capture)
- Task 4 depends on Task 1 (overlay needs monitor positions)
- Task 6 depends on Tasks 3, 4, 5 (integration requires all components)
- Task 7 requires all previous tasks

## Parallelizable Work

- Tasks 1 and 3 can be worked in parallel (backend enumeration and frontend UI)
- Tasks 4 and 5 can be worked in parallel (overlay logic and styling)
