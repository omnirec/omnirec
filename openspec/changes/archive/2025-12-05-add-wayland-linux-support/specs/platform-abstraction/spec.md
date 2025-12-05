## ADDED Requirements

### Requirement: Hyprland Backend Selection

The Linux backend SHALL detect and initialize Hyprland-specific capture when available.

#### Scenario: Backend selection on Wayland/Hyprland

- **WHEN** the application starts on Linux with Hyprland compositor
- **THEN** the `LinuxBackend` detects Hyprland via `HYPRLAND_INSTANCE_SIGNATURE`
- **AND** all capture operations use PipeWire via xdg-desktop-portal
- **AND** window/monitor enumeration uses Hyprland IPC

#### Scenario: Unsupported Linux environment

- **WHEN** the application starts on Linux without Hyprland
- **THEN** an error message is displayed: "This application requires Hyprland compositor"
- **AND** capture functionality is disabled

### Requirement: Window Handle Mapping for Wayland

The system SHALL map Hyprland window addresses to the shared `WindowInfo.handle` field.

#### Scenario: Window info on Wayland/Hyprland

- **WHEN** windows are enumerated on Hyprland
- **THEN** `WindowInfo.handle` contains the Hyprland window address as isize
- **AND** `WindowInfo.title` contains the window title
- **AND** `WindowInfo.process_name` contains the application class
