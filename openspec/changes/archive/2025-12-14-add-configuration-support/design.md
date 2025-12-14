## Context

OmniRec currently saves recordings to a hardcoded location (user's Videos folder). Users need the ability to customize the output directory, and the app needs a general configuration system to support this and future settings.

**Constraints:**
- Must work across Windows, macOS, and Linux
- Config file must be in platform-standard location
- No cloud/network dependencies (local file storage only)
- UI must fit within the existing fixed-size window

## Goals / Non-Goals

**Goals:**
- Provide a clean configuration UI accessible from the main window
- Support grouped settings for organization
- Persist settings to platform-appropriate config directory
- Allow output directory customization with folder picker support

**Non-Goals:**
- Import/export of settings
- Multiple configuration profiles
- Cloud sync of settings
- Real-time config file watching (restart not required, but live editing not supported)

## Decisions

### Config File Location
- **Decision:** Use platform-standard config directories
  - Linux: `~/.config/omnirec/config.json`
  - macOS: `~/Library/Application Support/omnirec/config.json`
  - Windows: `%APPDATA%\omnirec\config.json`
- **Rationale:** Follows platform conventions, easy to find/backup, Tauri provides `app_config_dir()` helper

### Config File Format
- **Decision:** JSON format
- **Rationale:** Human-readable, easy to parse in Rust with serde, widely understood

### Config Schema
```json
{
  "output": {
    "directory": "/custom/path/to/videos"
  }
}
```
- **Rationale:** Grouped by feature area, extensible for future settings within each group

### UI Integration
- **Decision:** Gear icon button in the tab bar, right-aligned, acts as a tab
- **Rationale:** Consistent with existing tab navigation pattern, easily discoverable

### Default Value Display
- **Decision:** Show system-detected default as grayed placeholder text in the input field
- **Rationale:** Users see what the default is without it being "set" as their preference

### Folder Picker
- **Decision:** Use Tauri's native dialog API for folder selection
- **Rationale:** Platform-native experience, handles permissions correctly

### Auto-Save Behavior
- **Decision:** Settings auto-save on change, no explicit save button
- **Rationale:** Reduces friction, modern UX pattern, immediate feedback
- **Implementation:**
  - Text inputs: Save on blur (focus loss) or after 500ms debounce of no typing
  - Folder picker: Save immediately after selection
  - Show inline validation errors, revert to previous value if validation fails

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Config file corruption | Validate JSON on load, fall back to defaults on parse error |
| Invalid directory path | Validate directory exists and is writable before saving, show error if not |
| Config dir doesn't exist | Create directory on first write |

## Migration Plan

- No migration needed (new feature)
- Existing users continue using system default until they configure a custom path

## Open Questions

None - all clarified in discussion.
