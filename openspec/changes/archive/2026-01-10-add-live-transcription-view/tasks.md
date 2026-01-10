## 1. Backend - Event Emission for Transcription Segments

- [x] 1.1 Add `transcription-segment` event type to `omnirec-common` crate with segment payload (timestamp, text)
- [x] 1.2 Modify `TranscriptionQueue` worker to emit events via callback when segments are transcribed
- [x] 1.3 Add polling endpoint to service for frontend to fetch segments (used instead of event streaming)

## 2. Configuration - Show Transcript Setting

- [x] 2.1 Add `show_transcript_window` boolean field to `TranscriptionConfig` in `src-tauri/src/config.rs` (default: true)
- [x] 2.2 Update `get_transcription_config` and `save_transcription_config` commands to handle new field
- [x] 2.3 Add "Show transcript when recording starts" checkbox to configuration view in `src/main.ts`
- [x] 2.4 Add checkbox HTML element to `index.html` in the transcription settings area

## 3. Transcription Window - Frontend

- [x] 3.1 Create `src/transcript-view.html` with basic structure for the transcription window
- [x] 3.2 Create `src/transcript-view.css` using theme variables from `styles.css` (tall narrow layout, scrollable content)
- [x] 3.3 Create `src/transcript-view.ts` to poll for segments and display them
- [x] 3.4 Window is created dynamically via WebviewWindowBuilder (not pre-defined in tauri.conf.json)

## 4. Window Management - Backend

- [x] 4.1 Add `open_transcript_window` Tauri command to create/show the transcript window
- [x] 4.2 Add `close_transcript_window` Tauri command to close the window
- [x] 4.3 Register new commands in `src-tauri/src/lib.rs` invoke_handler

## 5. Recording Integration

- [x] 5.1 Modify `startRecording()` in `src/main.ts` to open transcript window when transcription is enabled and setting is on
- [x] 5.2 Transcript window polls for segments via `get_transcription_segments` command
- [x] 5.3 Clear transcript window content when a new recording starts (handled via polling with sinceIndex=0)

## 6. Testing and Validation

- [x] 6.1 Test transcript window opens automatically when recording starts with transcription enabled
- [x] 6.2 Test transcript segments appear in real-time during recording
- [x] 6.3 Test window respects theme (light/dark mode)
- [x] 6.4 Test window is resizable and movable
- [x] 6.5 Test setting disabled prevents automatic window opening
- [x] 6.6 Run `cargo clippy` and `pnpm exec tsc --noEmit` to verify no linting errors
