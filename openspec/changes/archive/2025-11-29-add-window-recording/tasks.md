# Tasks: Add Basic Window Recording

## 1. Project Setup

- [x] 1.1 Add Rust dependencies to `Cargo.toml`:
  - `windows-capture` for frame capture
  - `ffmpeg-sidecar` for video encoding
  - `tokio` for async runtime (if not already present)
  - `directories` for finding Videos folder
- [x] 1.2 Initialize ffmpeg-sidecar to download/bundle FFmpeg binary
- [x] 1.3 Add `@tauri-apps/api` to frontend dependencies (if missing)

## 2. Window Enumeration (Backend)

- [x] 2.1 Create `src-tauri/src/capture/mod.rs` module structure
- [x] 2.2 Implement `list_windows()` function using Windows API
  - Enumerate visible windows with `EnumWindows`
  - Filter out system windows, invisible windows, minimized windows
  - Return list with window handle, title, and process name
- [x] 2.3 Create Tauri command `get_windows` that returns window list to frontend
- [x] 2.4 Define `WindowInfo` struct with serialization for frontend

## 3. Frame Capture (Backend)

- [x] 3.1 Create `src-tauri/src/capture/recorder.rs` for capture logic
- [x] 3.2 Implement window capture using `windows-capture` crate
  - Initialize capture for selected window handle
  - Capture frames at 30 FPS
  - Convert frames to BGRA format
- [x] 3.3 Implement frame channel for capture → encoder communication
- [x] 3.4 Handle window close/resize events during capture

## 4. Video Encoding (Backend)

- [x] 4.1 Create `src-tauri/src/encoder/mod.rs` module structure
- [x] 4.2 Implement FFmpeg process spawning via ffmpeg-sidecar
  - Configure H.264 encoding (libx264)
  - Set MP4 container output
  - Pipe raw BGRA frames to FFmpeg stdin
- [x] 4.3 Generate output filename with timestamp in Videos folder
- [x] 4.4 Handle encoding completion and file finalization

## 5. Recording State Machine (Backend)

- [x] 5.1 Create `src-tauri/src/state.rs` for recording state management
- [x] 5.2 Implement state enum: `Idle`, `Recording`, `Saving`
- [x] 5.3 Create Tauri commands:
  - `start_recording(window_handle)` - Begin capture
  - `stop_recording()` - Stop capture and save
  - `get_recording_state()` - Return current state
- [x] 5.4 Emit Tauri events for state changes and errors

## 6. Frontend UI

- [x] 6.1 Update `index.html` with recording UI layout:
  - Window list container
  - Record/Stop button
  - Status display area
  - Recording timer
- [x] 6.2 Update `src/styles.css` with recording UI styles
- [x] 6.3 Update `src/main.ts`:
  - Fetch and display window list on load
  - Handle window selection
  - Implement record/stop button logic
  - Display recording timer during capture
  - Show save confirmation with file path

## 7. Integration & Testing

- [x] 7.1 Wire up all Tauri commands in `lib.rs`
- [ ] 7.2 Test window enumeration displays correctly
- [ ] 7.3 Test recording start/stop produces valid MP4
- [ ] 7.4 Test window close during recording handles gracefully
- [ ] 7.5 Verify output plays in Windows Media Player and VLC

## 8. Error Handling

- [x] 8.1 Add user-friendly error messages for common failures:
  - Window no longer exists
  - Permission denied for capture
  - FFmpeg encoding failure
  - Disk write failure
- [x] 8.2 Ensure graceful recovery to Idle state on any error

## Dependencies

- Tasks 2.x and 3.x can be developed in parallel
- Task 4.x depends on 3.3 (frame channel)
- Task 5.x depends on 3.x and 4.x
- Task 6.x can be developed in parallel with backend tasks
- Task 7.x requires all previous tasks complete
