## 1. macOS Tray Icon Assets

- [x] 1.1 Create macOS template icons (`omnirec-template.png`, `omnirec-template@2x.png`) - monochromatic icons that macOS will automatically tint based on menu bar appearance
- [x] 1.2 Create macOS template recording icons (`omnirec-recording-template.png`, `omnirec-recording-template@2x.png`) with red dot indicator

## 2. Cross-Platform Menu Updates

- [x] 2.1 Add `TRANSCRIPTION` to `menu_ids` in `src-tauri/src/tray/mod.rs`
- [x] 2.2 Add `TRANSCRIPTION` label to `menu_labels` in `src-tauri/src/tray/mod.rs`
- [x] 2.3 Add transcription menu item to Linux tray menu (between Stop Recording and Configuration)
- [x] 2.4 Add transcription menu item to Windows tray menu (between Stop Recording and Configuration)

## 3. macOS Tray Implementation

- [x] 3.1 Implement `setup_tray` in `src-tauri/src/tray/macos.rs`:
  - Create tray icon with monochromatic template icon
  - Build menu with all items (Record Screen/Window, Stop Recording, Transcription, Configuration, About, Exit)
  - Set up menu event handlers that emit Tauri events
- [x] 3.2 Implement `set_recording_state` in `src-tauri/src/tray/macos.rs`:
  - Update icon to recording indicator (red dot) when recording
  - Update icon to normal when not recording
  - Update menu item enabled states (Record disabled during recording, Stop disabled when idle)
- [x] 3.3 Implement `set_tray_visible` in `src-tauri/src/tray/macos.rs`
- [x] 3.4 Update `src-tauri/src/tray/mod.rs` to expose `set_recording_state` for macOS

## 4. Frontend Event Handling

- [x] 4.1 Add `tray-show-transcription` event listener in `src/main.ts`:
  - If transcription is active (recording with transcription enabled), show/activate transcript window
  - If transcription is not active, show error message via notification or dialog
- [x] 4.2 Emit `tray-show-transcription` event from all platform tray implementations

## 5. Testing and Validation

- [ ] 5.1 Verify tray icon appears on macOS menu bar on startup
- [ ] 5.2 Verify icon adapts to light/dark menu bar theme
- [ ] 5.3 Verify recording indicator (red dot) appears during recording
- [ ] 5.4 Verify all menu items function correctly on macOS
- [ ] 5.5 Verify transcription menu item works on all platforms
- [x] 5.6 Run `cargo clippy` and fix any warnings
