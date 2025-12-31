# Tasks: Add Command-Line Interface

## 1. Project Setup

- [x] 1.1 Create `src-cli/` crate with Cargo.toml (dependencies: clap, tokio, serde_json, omnirec-common)
- [x] 1.2 Add src-cli to workspace in root Cargo.toml
- [x] 1.3 Set up basic binary entry point with clap argument parsing
- [x] 1.4 Add CLI binary to build scripts and CI workflows

## 2. Service Connection

- [x] 2.1 Implement IPC client connection (reuse omnirec-common IPC types)
- [x] 2.2 Implement service auto-start if not running
- [x] 2.3 Implement connection timeout and retry logic
- [x] 2.4 Add health check (ping/pong) on connect

## 3. List Commands

- [x] 3.1 Implement `list windows` subcommand
- [x] 3.2 Implement `list displays` subcommand
- [x] 3.3 Implement `list audio` subcommand
- [x] 3.4 Add `--json` output mode for all list commands
- [x] 3.5 Format human-readable output with aligned columns

## 4. Recording Commands

- [x] 4.1 Implement `record window <HANDLE>` subcommand
- [x] 4.2 Implement `record display <ID>` subcommand
- [x] 4.3 Implement `record region` with coordinate flags
- [x] 4.4 Implement `record portal` for Wayland portal mode
- [x] 4.5 Add `--output`, `--format`, `--duration` flags
- [x] 4.6 Add `--audio` and `--microphone` flags

## 5. Control Commands

- [x] 5.1 Implement `stop` subcommand
- [x] 5.2 Implement `status` subcommand
- [x] 5.3 Implement `version` subcommand

## 6. Event Handling

- [x] 6.1 Subscribe to service events after starting recording
- [x] 6.2 Display elapsed time updates (unless --quiet)
- [x] 6.3 Handle transcoding progress events
- [x] 6.4 Display final file path on completion

## 7. Signal Handling

- [x] 7.1 Handle SIGINT (Ctrl+C) to gracefully stop recording
- [x] 7.2 Handle SIGTERM for graceful shutdown
- [x] 7.3 Ensure file is saved before CLI exits

## 8. Platform Handling

- [x] 8.1 Detect Wayland portal-mode desktops
- [x] 8.2 Show warning when specific targets unavailable on portal-mode
- [x] 8.3 Fall back to portal for window/display/region on portal-mode
- [x] 8.4 Test on Windows, macOS, and Linux (Hyprland + GNOME)

## 9. Exit Codes

- [x] 9.1 Implement structured exit codes per design doc
- [x] 9.2 Document exit codes in help text

## 10. Testing

- [x] 10.1 Unit tests for argument parsing
- [x] 10.2 Integration tests for list commands (mock service)
- [x] 10.3 Manual testing of recording workflow on each platform
- [x] 10.4 Test duration auto-stop functionality

## 11. Packaging

- [x] 11.1 Update release workflow to build CLI binary
- [x] 11.2 Include CLI in platform packages (MSI, DMG, DEB, RPM, AppImage)
- [x] 11.3 Update AUR PKGBUILD to include CLI
- [x] 11.4 Update README with CLI usage examples

## 12. Documentation

- [x] 12.1 Add CLI section to README
- [x] 12.2 Document all subcommands and flags
- [x] 12.3 Provide usage examples for common workflows
- [x] 12.4 Document Wayland restrictions and portal fallback
