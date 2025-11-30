# Tasks: Add Full-Screen Display Recording

## 1. Backend: Display Capture Command

- [x] 1.1 Add `start_display_recording` Tauri command in `lib.rs` that accepts a monitor ID
- [x] 1.2 Implement display recording by reusing `region_recorder` with full monitor dimensions (x=0, y=0, width=monitor.width, height=monitor.height)
- [x] 1.3 Add unit test to verify display capture starts correctly

## 2. Frontend: UI Mode Toggle

- [x] 2.1 Add "Display" button to mode toggle in `index.html`
- [x] 2.2 Add `mode-display-btn` event listener in `main.ts`
- [x] 2.3 Update `CaptureMode` type to include `"display"`
- [x] 2.4 Update `setCaptureMode` function to handle display mode

## 3. Frontend: Display Selection UI

- [x] 3.1 Add display selection section in `index.html` (hidden by default)
- [x] 3.2 Add display list container and dropdown/list styling in `styles.css`
- [x] 3.3 Implement `loadDisplays` function in `main.ts` to fetch and render display list
- [x] 3.4 Implement display item click handler to select a display
- [x] 3.5 Add `selectedDisplay` state variable

## 4. Frontend: Recording Integration

- [x] 4.1 Update `startRecording` function to handle display mode
- [x] 4.2 Update `updateRecordButton` to check `selectedDisplay` in display mode
- [x] 4.3 Update `disableSelection` to include display list controls
- [x] 4.4 Update status messages for display mode

## 5. Validation

- [ ] 5.1 Manual test: Verify display list shows all connected monitors
- [ ] 5.2 Manual test: Verify selecting a display enables Record button
- [ ] 5.3 Manual test: Verify recording captures entire display
- [ ] 5.4 Manual test: Verify mode switching hides/shows correct sections
- [x] 5.5 Run `cargo clippy` and fix any warnings
- [x] 5.6 Run `cargo test` and ensure all tests pass
