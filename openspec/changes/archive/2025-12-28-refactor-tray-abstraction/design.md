# Design: Tray Abstraction

## Context

OmniRec currently implements system tray as a Linux-only feature for "tray-mode" desktops (GNOME, KDE, COSMIC) where the portal handles source selection. The tray code is tightly coupled to Linux-specific logic and naming (`GnomeTrayState`, `is_tray_mode_desktop()`).

However, system tray is a standard feature on all major desktop platforms:
- **Windows**: System tray in taskbar notification area
- **macOS**: Menu bar status items
- **Linux**: App indicators / system tray (requires extension on some DEs)

Abstracting the tray functionality enables:
1. Consistent quick-access recording controls across all platforms
2. Cleaner separation between "tray as a feature" and "portal-mode workflow"
3. Foundation for future enhancements (recording indicator, quick settings)

## Goals

- Provide cross-platform tray icon with recording controls
- Refactor existing Linux tray code into shared abstraction
- Create stubs for Windows and macOS with clear implementation path
- Maintain existing portal-mode behavior for GNOME/KDE/COSMIC

## Non-Goals

- Fully implement Windows/macOS tray in this change (stubs only)
- Change the portal-mode workflow for Linux DEs
- Add new tray menu items beyond current set

## Decisions

### Decision: Separate system-tray from gnome-tray-mode specs

**What**: Create new `system-tray` capability for cross-platform tray abstraction. Keep `gnome-tray-mode` for Linux portal-mode specific behavior.

**Why**:
- System tray is a general feature; portal-mode is a Linux DE workaround
- Clear separation of concerns
- `gnome-tray-mode` can reference `system-tray` for shared behavior

### Decision: Module organization with platform subdirectories

**What**: Structure tray code as:
```
src-tauri/src/tray/
├── mod.rs      # Shared types, TrayManager trait
├── linux.rs    # Linux implementation
├── windows.rs  # Windows stub
└── macos.rs    # macOS stub
```

**Why**:
- Mirrors existing `capture/` module organization
- Clear platform separation with compile-time selection
- Easy to extend each platform independently

### Decision: TrayManager abstraction

**What**: Define a `TrayManager` struct/trait with platform-agnostic interface:
- `setup_tray()` - Initialize tray icon and menu
- `set_recording_state()` - Update icon based on recording
- `set_visible()` - Show/hide tray icon
- Recording state tracking

**Why**:
- Commands can use a single interface regardless of platform
- Platform-specific behavior encapsulated in implementations
- Easier testing with trait-based design

### Decision: Tray enabled by default (with platform-specific activation)

**What**: Tray initialization runs on all platforms. Each platform decides:
- **Windows/macOS**: Tray always visible, main window close behavior unchanged
- **Linux portal-mode**: Tray visible, main window hidden on startup, close hides window

**Why**:
- Users expect tray on all platforms
- Platform-specific window lifecycle can be handled independently
- Consistent codebase structure

### Decision: Stub implementations for Windows/macOS

**What**: Initial Windows/macOS implementations return success but don't create actual tray icons.

**Why**:
- Enables code structure refactor without blocking on full implementation
- Allows compilation and testing on all platforms
- Clear path for future implementation

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Application                              │
├─────────────────────────────────────────────────────────────────┤
│                    TrayManager (shared types)                    │
│  - TrayState { is_recording, tray_handle }                      │
│  - setup_tray(app) -> Result                                    │
│  - set_recording_state(recording) -> Result                     │
│  - set_visible(visible) -> Result                               │
├───────────────┬───────────────┬─────────────────────────────────┤
│ linux.rs      │ windows.rs    │ macos.rs                        │
│ - Full impl   │ - Stub        │ - Stub                          │
│ - Icon load   │ - No-op       │ - No-op                         │
│ - Menu setup  │               │                                 │
│ - Events      │               │                                 │
└───────────────┴───────────────┴─────────────────────────────────┘

Tray Menu (consistent across platforms):
┌─────────────────────┐
│ Start Recording     │ ← Behavior varies by mode
├─────────────────────┤
│ Stop Recording      │ ← Universal
├─────────────────────┤
│ Configuration       │ ← Shows window
├─────────────────────┤
│ About               │ ← Shows window
├─────────────────────┤
│ Exit                │ ← Quits app
└─────────────────────┘

Platform Behavior Matrix:
┌──────────────┬────────────┬─────────────┬────────────────────────┐
│ Platform     │ Tray       │ Window on   │ Close Button           │
│              │ Default    │ Startup     │ Behavior               │
├──────────────┼────────────┼─────────────┼────────────────────────┤
│ Windows      │ Enabled    │ Visible     │ Minimizes to tray      │
│ macOS        │ Enabled    │ Visible     │ Minimizes to tray      │
│ Linux/Hypr   │ Enabled    │ Visible     │ Closes app             │
│ Linux/GNOME  │ Enabled    │ Hidden      │ Hides (portal-mode)    │
│ Linux/KDE    │ Enabled    │ Hidden      │ Hides (portal-mode)    │
│ Linux/COSMIC │ Enabled    │ Hidden      │ Hides (portal-mode)    │
└──────────────┴────────────┴─────────────┴────────────────────────┘
```

## Refactoring Plan

1. **Create new module structure** - `tray/mod.rs`, `tray/linux.rs`, `tray/windows.rs`, `tray/macos.rs`
2. **Extract shared types** - `TrayState`, menu constants, icon paths
3. **Move Linux code** - Migrate from `tray.rs` to `tray/linux.rs`
4. **Rename types** - `GnomeTrayState` → `TrayState`, `is_tray_mode_desktop()` → `is_portal_mode()`
5. **Create stubs** - Windows and macOS return success, log messages
6. **Update lib.rs** - Call unified `tray::setup()` on all platforms
7. **Update commands** - Use platform-agnostic tray state access

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Windows/macOS stubs provide no user value | Document as future work; focus on refactor |
| Breaking existing Linux tray behavior | Careful refactor; test on GNOME/KDE |
| Naming confusion (gnome-tray-mode vs system-tray) | Clear spec purposes and cross-references |

## Migration Plan

This is a refactor of internal code structure. No user-visible migration needed.

## Open Questions

- Should Windows/macOS close button behavior differ from Linux non-portal mode?
- Should tray menu items be customizable per platform?
