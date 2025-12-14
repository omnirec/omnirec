## 1. Backend Configuration System

- [x] 1.1 Create `src-tauri/src/config.rs` module with Config struct and serde serialization
- [x] 1.2 Implement `load_config()` function to read from platform config directory
- [x] 1.3 Implement `save_config()` function to write config file
- [x] 1.4 Implement `get_default_output_dir()` to return system Videos folder path
- [x] 1.5 Add Tauri commands: `get_config`, `save_config`, `get_default_output_dir`, `pick_output_directory`
- [x] 1.6 Update `encoder/mod.rs` to use configured output directory with fallback to default

## 2. Frontend Configuration UI

- [x] 2.1 Add gear icon button to tab bar in `index.html`, right-aligned
- [x] 2.2 Add configuration view section in `index.html` with Output group and directory input
- [x] 2.3 Add folder picker button next to directory input
- [x] 2.4 Style config tab button in `styles.css` (gear icon, right-aligned positioning)
- [x] 2.5 Style configuration view in `styles.css` (groups, inputs, default placeholder)
- [x] 2.6 Implement config tab switching logic in `main.ts`
- [x] 2.7 Load config on app start and populate UI
- [x] 2.8 Implement directory input auto-save (debounced on typing, immediate on blur)
- [x] 2.9 Implement folder picker button handler using Tauri dialog (auto-saves on selection)

## 3. Validation and Polish

- [x] 3.1 Add directory validation (exists, writable) before saving
- [x] 3.2 Display validation errors in UI
- [x] 3.3 Test on Linux, verify config file location and persistence
- [ ] 3.4 Test on macOS (if available), verify config file location
- [ ] 3.5 Test on Windows (if available), verify config file location
