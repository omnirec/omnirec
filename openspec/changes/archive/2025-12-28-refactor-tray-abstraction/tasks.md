# Tasks: Refactor Tray Abstraction

## 1. Create Module Structure

- [x] 1.1 Create `src-tauri/src/tray/mod.rs` with shared types and exports
- [x] 1.2 Create `src-tauri/src/tray/linux.rs` placeholder
- [x] 1.3 Create `src-tauri/src/tray/windows.rs` stub
- [x] 1.4 Create `src-tauri/src/tray/macos.rs` stub

## 2. Define Shared Types and Interface

- [x] 2.1 Define `TrayState` struct (rename from `GnomeTrayState`)
- [x] 2.2 Define tray menu constants (item IDs, labels)
- [x] 2.3 Define icon path resolution utilities
- [x] 2.4 Export unified `setup_tray()` and `set_tray_visible()` functions

## 3. Migrate Linux Implementation

- [x] 3.1 Move Linux tray code from `tray.rs` to `tray/linux.rs`
- [x] 3.2 Rename `is_tray_mode_desktop()` to `is_portal_mode()`
- [x] 3.3 Update icon loading to use shared utilities
- [x] 3.4 Update menu creation to use shared constants

## 4. Implement Windows Stub

- [x] 4.1 Implement `setup_tray()` that returns Ok(()) with log message
- [x] 4.2 Implement `set_tray_visible()` as no-op
- [x] 4.3 Implement `set_recording_state()` as no-op (not needed - handled via set_tray_visible)

## 5. Implement macOS Stub

- [x] 5.1 Implement `setup_tray()` that returns Ok(()) with log message
- [x] 5.2 Implement `set_tray_visible()` as no-op
- [x] 5.3 Implement `set_recording_state()` as no-op (not needed - handled via set_tray_visible)

## 6. Update Application Integration

- [x] 6.1 Update `lib.rs` to call `tray::setup_tray()` on all platforms
- [x] 6.2 Update `commands/recording.rs` to use renamed `TrayState`
- [x] 6.3 Remove `#[cfg(target_os = "linux")]` guards from lib.rs tray exports
- [x] 6.4 Remove old `src-tauri/src/tray.rs` file

## 7. Update Frontend (if needed)

- [x] 7.1 Rename any references from "gnome-tray" to "portal-mode" in TypeScript (none found)
- [x] 7.2 Update event names if they reference GNOME-specific terminology (none found - events are generic)

## 8. Testing and Validation

- [x] 8.1 Run `cargo clippy` to verify no warnings
- [x] 8.2 Run `cargo test` to verify compilation
- [ ] 8.3 Test on Linux GNOME/KDE to verify tray still works (requires manual testing)
- [ ] 8.4 Test on Windows to verify stub doesn't break startup (requires manual testing)
- [ ] 8.5 Test on macOS to verify stub doesn't break startup (requires manual testing)
