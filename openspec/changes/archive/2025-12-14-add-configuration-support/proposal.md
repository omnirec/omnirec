# Change: Add Configuration Support

## Why

Users need the ability to customize where recordings are saved rather than relying solely on the system default Videos folder. A configuration system provides a foundation for future user preferences while keeping the initial scope minimal.

## What Changes

- Add a gear icon button to the right side of the capture mode tab bar
- Clicking the gear button switches to a Configuration tab view (replacing the capture UI)
- Configuration view displays settings organized by groups
- Initial "Output" group contains output directory setting
- Output directory shows detected system default as placeholder (grayed out)
- Users can type a custom path or use a folder picker to override the default
- Custom output directory persists to a config file in the platform-standard config directory
- Backend uses custom output directory when set, falling back to system default

## Impact

- Affected specs: ui-theme (tab bar modification), new app-configuration capability
- Affected code:
  - `src/main.ts` - Config tab UI, state management
  - `src/styles.css` - Config view styling
  - `index.html` - Config tab button and view markup
  - `src-tauri/src/lib.rs` - Config Tauri commands
  - `src-tauri/src/encoder/mod.rs` - Use configured output directory
  - New config module in Rust backend for persistence
