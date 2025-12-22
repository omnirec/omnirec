# Change: Add Approval Dialog to OmniRec Picker

## Why

Currently, when OmniRec is running with an active recording request, the picker auto-approves portal screencast requests without any user confirmation. This bypasses the consent flow that users expect when an application requests screen recording permission. An approval dialog ensures users explicitly grant permission before recording begins, while still providing a streamlined experience for trusted scenarios.

## What Changes

- Add a graphical approval dialog to `omnirec-picker` that appears when OmniRec has a pending recording request
- Dialog asks: "Allow OmniRec to record the screen?" with Allow/Deny buttons
- Add "Always allow OmniRec to record the screen" checkbox for persistent permission
- Implement OmniRec-specific approval token system (separate from portal restore tokens):
  - Picker generates token when user checks "Always allow"
  - Token sent to OmniRec app via IPC
  - OmniRec stores token in state directory
  - Picker queries token before showing dialog; valid token bypasses dialog
- Fallback to standard picker unchanged (when OmniRec not running or no pending request)

## Impact

- Affected specs: `wayland-portal`
- Affected code:
  - `src-picker/src/main.rs` - Add dialog display and token logic
  - `src-picker/src/ipc_client.rs` - Add token query/store IPC messages
  - `src-tauri/src/capture/linux/ipc_server.rs` - Handle token storage/retrieval
  - `src-picker/Cargo.toml` - Add GUI dependency (gtk4 or similar)
