# Design: Picker Approval Dialog

## Context

The `omnirec-picker` is invoked by xdg-desktop-portal-hyprland (XDPH) when a screencast request requires source selection. Currently, if OmniRec is running with an active selection, the picker auto-approves immediately. This change adds a user consent step while maintaining the streamlined UX for trusted/repeated use.

### Stakeholders
- End users who expect explicit consent before screen recording
- OmniRec application needing seamless recording initiation
- Other applications (OBS, Zoom, etc.) that should use the standard picker

## Goals / Non-Goals

### Goals
- User must explicitly approve OmniRec's first recording request
- Provide "always allow" option to skip dialog for trusted use
- Maintain fallback to standard picker for non-OmniRec requests
- Token system independent of portal restore tokens (OmniRec-specific)

### Non-Goals
- Per-source approval (e.g., allow DP-1 but not DP-2) - all sources treated equally
- Integration with system permission stores (e.g., Flatpak portals)
- Revoking permission from OmniRec UI (user deletes token file manually or via future settings)

## Decisions

### Decision: Use GTK4 for the dialog
- **Rationale**: Native Linux look, widely available, good Rust bindings (gtk4-rs)
- **Alternatives considered**:
  - zenity/kdialog: Simpler but external dependency, less control over appearance
  - egui: Would add significant binary size, non-native look
  - slint: Good option but less ecosystem support than GTK

### Decision: Simple random token stored in XDG state directory
- **Rationale**: 
  - Token is OmniRec-specific, not a portal restore token
  - Random 256-bit token provides sufficient entropy
  - XDG state dir (`$XDG_STATE_HOME/omnirec/` or `~/.local/state/omnirec/`) is appropriate for runtime state that should persist but isn't config
- **Token format**: 64-character hex string stored in `approval-token` file
- **Validation**: Picker sends token to OmniRec app, app confirms it matches stored token

### Decision: Token validation via IPC round-trip
- **Rationale**: 
  - Picker cannot directly read OmniRec's state directory (different process, potentially different user context in Flatpak)
  - IPC already exists for selection queries
  - App is authoritative source of token validity
- **Flow**: Picker asks app "is this token valid?" via IPC before showing dialog

### Decision: Blocking dialog display
- **Rationale**: XDPH expects picker to output selection to stdout and exit; dialog must block until user responds
- GTK main loop runs until user clicks Allow/Deny

## Component Interaction

```
Portal Request Flow (OmniRec running with pending request):

XDPH                    omnirec-picker              OmniRec App
  |                           |                          |
  |--invoke picker----------->|                          |
  |                           |--IPC: QuerySelection---->|
  |                           |<--Selection + has_token--|
  |                           |                          |
  |                           |--IPC: ValidateToken----->| (if has_token)
  |                           |<--TokenValid/Invalid-----|
  |                           |                          |
  |                           | [If token invalid/missing]
  |                           | [Show GTK dialog]        |
  |                           |   User clicks Allow      |
  |                           |   [x] Always allow       |
  |                           |                          |
  |                           |--IPC: StoreToken-------->| (if always allow checked)
  |                           |<--TokenStored------------|
  |                           |                          |
  |<--stdout: [SELECTION]/----|                          |
  |                           |                          |
```

## IPC Protocol Extensions

New message types:

```rust
// Request
enum IpcRequest {
    QuerySelection,           // existing
    ValidateToken { token: String },  // new
    StoreToken { token: String },     // new
}

// Response  
enum IpcResponse {
    Selection { ... },        // existing - add optional `has_approval_token: bool`
    NoSelection,              // existing
    Error { message: String },// existing
    TokenValid,               // new
    TokenInvalid,             // new
    TokenStored,              // new
}
```

## Token Storage

Location: `$XDG_STATE_HOME/omnirec/approval-token` (typically `~/.local/state/omnirec/approval-token`)

Format: Plain text file containing 64-character hex string (256-bit random token)

File permissions: `0600` (owner read/write only)

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| GTK dependency increases picker binary size | Accept trade-off; GTK is commonly installed on Linux desktops |
| Dialog may not match desktop theme | Use default GTK theme detection; libadwaita for modern GNOME look |
| Token file could be copied to another machine | Token only works with running OmniRec instance that generated it; low risk |
| User might forget they enabled "always allow" | Future: add revoke option in OmniRec settings |

## Open Questions

- Should the dialog show which source (monitor/window) is being requested? (Leaning yes for transparency)
- Should there be a timeout on the dialog? (Leaning no - user should take their time)
