# Design: GNOME Desktop Support

## Context

OmniRec currently supports Hyprland on Linux by implementing a custom xdg-desktop-portal picker that auto-approves capture requests based on the user's selection in the main UI. This approach doesn't work on GNOME because:

1. GNOME's portal implementation doesn't support external picker overrides
2. The portal always shows its own native picker dialog
3. Users cannot pre-select a capture target in OmniRec that the portal will respect

This means the current UI-first workflow (select target -> click record -> portal auto-approves) cannot work on GNOME.

## Goals

- Provide a functional recording workflow on GNOME desktop
- Minimize user friction by leveraging native GNOME portal picker
- Keep the backend running for quick repeated recordings
- Maintain cross-platform consistency where possible (About tab)

## Non-Goals

- Override or bypass GNOME's portal picker
- Support other Wayland compositors (KDE, sway) in this change
- Implement system audio capture on GNOME (existing limitation)

## Decisions

### Decision: Tray-based workflow for GNOME

**What**: Instead of showing the main window on launch, GNOME mode adds a tray icon. Users control recording via tray menu.

**Why**: 
- The main window's capture target selection is useless on GNOME (portal ignores it)
- Tray icon provides quick access without window clutter
- Matches common recording app patterns (OBS, SimpleScreenRecorder)

**Alternatives considered**:
- Show main window anyway with a "Start Recording" button that opens portal picker
  - Rejected: Extra click, confusing UI with non-functional tabs
- Always show portal picker on app launch
  - Rejected: Poor UX, no way to configure before recording

### Decision: Use Tauri's tray-icon plugin

**What**: Use `tauri-plugin-tray-icon` for system tray integration.

**Why**:
- Already in Cargo.lock (dependency exists)
- Cross-platform support if we want tray on other platforms later
- Maintained as part of Tauri ecosystem

### Decision: Hide capture mode tabs on GNOME

**What**: On GNOME, hide Window/Region/Display tabs; only show Config and About.

**Why**:
- These tabs serve no purpose on GNOME (portal handles selection)
- Reduces confusion about why selections don't affect recording
- Cleaner, more focused UI

### Decision: Window close hides instead of quits on GNOME

**What**: On GNOME, clicking the window close button hides the window but keeps the app running.

**Why**:
- Tray icon remains available for recording control
- Consistent with tray-based app behavior (Discord, Slack, etc.)
- Exit menu item provides explicit quit

### Decision: Separate About tab (not in Config)

**What**: About is a separate tab alongside Config, not a section within Config.

**Why**:
- Config tab is for user-modifiable settings
- About is informational, doesn't belong mixed with settings
- Cleaner separation of concerns

## Architecture

```
GNOME Detection Flow:
┌─────────────────┐
│ App Startup     │
└────────┬────────┘
         │
         ▼
┌─────────────────┐     Yes    ┌─────────────────┐
│ Is GNOME?       │───────────►│ Tray Mode       │
│ (XDG_CURRENT_   │            │ - Add tray icon │
│  DESKTOP=GNOME) │            │ - Hide window   │
└────────┬────────┘            │ - Hide capture  │
         │ No                  │   tabs          │
         ▼                     └─────────────────┘
┌─────────────────┐
│ Normal Mode     │
│ (Hyprland/etc)  │
└─────────────────┘

Tray Menu Structure:
┌─────────────────────┐
│ Start Recording     │ ← Opens GNOME portal picker
├─────────────────────┤
│ Stop Recording      │ ← Grayed when not recording
├─────────────────────┤
│ Configuration       │ ← Shows window, Config tab
├─────────────────────┤
│ About               │ ← Shows window, About tab
├─────────────────────┤
│ Exit                │ ← Quits app, removes tray
└─────────────────────┘

Recording State → Tray Icon:
- Idle: Normal app icon
- Recording: Red dot icon
```

## GNOME Detection

Detect GNOME via environment variables:
```rust
fn is_gnome() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.to_uppercase().contains("GNOME"))
        .unwrap_or(false)
}
```

Note: This detects pure GNOME. GNOME-based desktops (Ubuntu's modified GNOME, Pop!_OS, etc.) should also match via the "GNOME" substring.

## Recording Flow on GNOME

1. User clicks "Start Recording" in tray menu
2. App calls portal's ScreenCast.CreateSession / SelectSources / Start
3. GNOME's native picker appears (user selects window/screen/region)
4. Portal returns PipeWire node ID
5. Recording proceeds as normal (existing capture pipeline)
6. Tray icon changes to red dot
7. User clicks "Stop Recording" in tray
8. Recording stops, file saved, tray icon returns to normal

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Tray icon may not appear on some GNOME setups | Document requirement for AppIndicator/extension |
| Users may not notice tray icon | Show brief notification on first launch |
| No pre-selection means less predictable captures | This is inherent to GNOME's design; document limitation |

## Migration Plan

No migration needed - this is additive functionality for a newly supported platform.

## Open Questions

- Should we show a first-run notification explaining the tray workflow?
- Should "Start Recording" be disabled during the portal picker flow to prevent double-invocation?
