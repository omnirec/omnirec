# Change: Add Command-Line Interface

## Why

OmniRec currently requires the GUI to initiate recordings. Users need headless recording for automation, scripting, and remote workflows. A CLI will enable unattended recordings, integration with shell scripts, and use cases where a GUI is unavailable or undesirable.

## What Changes

- Add a new `omnirec` CLI binary alongside the existing service and GUI binaries
- Implement subcommands for listing capture sources (`list windows`, `list displays`, `list audio`)
- Implement recording commands for window, display, and region capture
- Support all existing output formats and audio configuration via CLI flags
- Handle platform-specific restrictions:
  - On Wayland portal-mode desktops (GNOME, KDE, COSMIC, Cinnamon), window/display/region recording falls back to portal-based selection
  - Provide clear messaging when specific targets cannot be selected programmatically
- Provide JSON output mode for scriptable automation
- Use appropriate exit codes for scripting integration

## Impact

- Affected specs: None (new capability)
- New spec: `command-line-interface`
- Affected code:
  - New crate: `src-cli/` (standalone CLI binary)
  - Reuses: `src-common/` IPC protocol and types
  - Communicates with: `omnirec-service` via existing IPC
- Build/packaging: CI workflows need to build and package the CLI binary
