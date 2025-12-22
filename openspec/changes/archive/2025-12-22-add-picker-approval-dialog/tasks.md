# Tasks: Add Picker Approval Dialog

## 1. Extend IPC Protocol

- [x] 1.1 Add new IPC message types to `src-picker/src/ipc_client.rs`:
  - `ValidateToken { token: String }` request
  - `StoreToken { token: String }` request
  - `TokenValid`, `TokenInvalid`, `TokenStored` responses
- [x] 1.2 Add `has_approval_token: bool` field to `Selection` response
- [x] 1.3 Mirror IPC types in `src-tauri/src/capture/linux/ipc_server.rs`
- [x] 1.4 Add unit tests for new IPC message serialization/deserialization

## 2. Implement Token Storage in OmniRec

- [x] 2.1 Create token storage module in `src-tauri/src/capture/linux/` with functions:
  - `get_token_path() -> PathBuf` (XDG_STATE_HOME handling)
  - `read_token() -> Option<String>`
  - `write_token(token: &str) -> Result<()>`
  - `has_token() -> bool`
- [x] 2.2 Implement IPC handlers for `ValidateToken` and `StoreToken` in `ipc_server.rs`
- [x] 2.3 Update `QuerySelection` handler to include `has_approval_token` in response
- [x] 2.4 Add unit tests for token storage functions

## 3. Add GTK4 Dialog to Picker

- [x] 3.1 Add `gtk4` dependency to `src-picker/Cargo.toml`
- [x] 3.2 Create `src-picker/src/dialog.rs` module with:
  - Dialog window with OmniRec branding/icon
  - "Allow OmniRec to record the screen?" message
  - "Always allow" checkbox
  - Allow/Deny buttons
- [x] 3.3 Implement `show_approval_dialog() -> DialogResult` function that:
  - Initializes GTK if needed
  - Shows modal dialog
  - Returns `Approved { always_allow: bool }` or `Denied`
- [ ] 3.4 Test dialog appearance on Hyprland (manual testing required)

## 4. Implement Token Generation

- [x] 4.1 Add `rand` dependency to `src-picker/Cargo.toml`
- [x] 4.2 Create `generate_approval_token() -> String` function (256-bit random, hex-encoded)
- [x] 4.3 Add helper function `validate_token_with_app(token: &str) -> bool` using IPC
- [x] 4.4 Add helper function `store_token_with_app(token: &str) -> Result<()>` using IPC

## 5. Integrate Dialog into Picker Flow

- [x] 5.1 Update `main.rs` to check for approval token after receiving selection:
  - Query if token exists (`has_approval_token` from selection response)
  - If exists, validate with `ValidateToken` IPC call
  - If valid, proceed to output selection (existing flow)
  - If invalid or missing, show approval dialog
- [x] 5.2 Handle dialog result:
  - If approved with "always allow", generate and store token via IPC
  - If approved without "always allow", proceed without storing
  - If denied, exit with failure
- [x] 5.3 Ensure fallback behavior unchanged (NoSelection or IPC failure → standard picker)

## 6. Testing and Validation

- [x] 6.1 Update `omnirec-picker-test` tool to display token status
- [ ] 6.2 Manual test: First launch shows dialog
- [ ] 6.3 Manual test: "Always allow" checkbox stores token and bypasses future dialogs
- [ ] 6.4 Manual test: Deny button cancels recording
- [ ] 6.5 Manual test: Non-OmniRec apps still get standard picker
- [ ] 6.6 Manual test: Delete token file, verify dialog reappears
- [x] 6.7 Run clippy and fix any warnings

## 7. Documentation

- [x] 7.1 Update README with approval dialog behavior
- [x] 7.2 Document token file location in user-facing docs
