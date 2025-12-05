## ADDED Requirements

### Requirement: Hyprland Environment Detection

The system SHALL detect when running on Hyprland compositor and initialize appropriately.

#### Scenario: Hyprland detected via environment

- **WHEN** the application starts on Linux
- **AND** the `HYPRLAND_INSTANCE_SIGNATURE` environment variable is set
- **THEN** the Hyprland/Wayland backend is activated
- **AND** connection to Hyprland IPC is established

#### Scenario: Non-Hyprland environment rejected

- **WHEN** the application starts on Linux
- **AND** `HYPRLAND_INSTANCE_SIGNATURE` is not set
- **THEN** an error is displayed: "This application requires Hyprland compositor"
- **AND** capture features are disabled

### Requirement: Hyprland Window Enumeration

The system SHALL enumerate windows using Hyprland IPC.

#### Scenario: List windows via Hyprland IPC

- **WHEN** the user requests the window list
- **THEN** the system queries Hyprland IPC for client list
- **AND** each window entry includes the window title
- **AND** each window entry includes the application class
- **AND** each window entry includes the Hyprland window address

#### Scenario: Filter special windows

- **WHEN** enumerating windows via Hyprland
- **THEN** windows with empty titles are excluded
- **AND** hidden windows are excluded
- **AND** the screen recorder's own windows are excluded

#### Scenario: Window list refresh

- **WHEN** the user requests a window list refresh
- **THEN** a fresh query is made to Hyprland IPC
- **AND** the list reflects current window state

### Requirement: Hyprland Monitor Enumeration

The system SHALL enumerate monitors/outputs using Hyprland IPC.

#### Scenario: List monitors via Hyprland IPC

- **WHEN** the user switches to display capture mode
- **THEN** the system queries Hyprland IPC for monitor list
- **AND** each monitor entry includes the output name
- **AND** each monitor entry includes resolution and position
- **AND** each monitor entry indicates if it is the focused monitor

### Requirement: PipeWire Video Capture

The system SHALL capture video frames via PipeWire streams.

#### Scenario: Receive video frames from PipeWire

- **WHEN** a PipeWire screencast stream is active
- **THEN** video frames are received at the stream's negotiated rate
- **AND** frames are converted to BGRA format for encoder compatibility
- **AND** frame dimensions match the captured source

#### Scenario: Handle PipeWire stream errors

- **WHEN** the PipeWire stream encounters an error
- **THEN** recording stops gracefully
- **AND** partial recording is saved if possible
- **AND** the user is notified of the error

#### Scenario: PipeWire buffer handling

- **WHEN** frames arrive faster than encoder can process
- **THEN** frames are dropped to prevent memory growth
- **AND** the newest frame is always processed

### Requirement: PipeWire Audio Capture

The system SHALL capture audio via PipeWire streams when audio recording is enabled.

#### Scenario: Capture system audio with video

- **WHEN** recording is started with audio enabled
- **THEN** the portal request includes audio capture
- **AND** audio samples are received via PipeWire
- **AND** audio is muxed with video in the output file

#### Scenario: Audio-only nodes available

- **WHEN** the user wants to record audio
- **THEN** available audio sources are listed (system audio, microphone)
- **AND** the user can select which audio sources to include

#### Scenario: Audio synchronization

- **WHEN** recording video with audio
- **THEN** audio and video timestamps are aligned
- **AND** the output file has synchronized audio/video tracks

### Requirement: Display Capture via PipeWire

The system SHALL capture entire displays using PipeWire screencast.

#### Scenario: Capture full display

- **WHEN** recording starts in display mode
- **AND** portal consent has been granted for the display
- **THEN** the PipeWire stream captures the entire monitor
- **AND** frame dimensions match the monitor resolution
- **AND** the cursor is included in the capture

### Requirement: Window Capture via PipeWire

The system SHALL capture individual windows using PipeWire screencast.

#### Scenario: Capture specific window

- **WHEN** recording starts in window mode
- **AND** portal consent has been granted for the window
- **THEN** the PipeWire stream captures only the selected window
- **AND** frame dimensions match the window size
- **AND** window resizing is reflected in the stream

#### Scenario: Window closed during capture

- **WHEN** the target window is closed during recording
- **THEN** the PipeWire stream ends
- **AND** recording stops automatically
- **AND** the partial recording is saved
- **AND** the user is notified

### Requirement: Region Capture via PipeWire

The system SHALL capture screen regions by cropping the PipeWire stream.

#### Scenario: Capture region from display stream

- **WHEN** recording starts in region mode
- **THEN** a full display stream is obtained via portal
- **AND** frames are cropped to the selected region
- **AND** output dimensions match the region size

#### Scenario: Region extends beyond display

- **WHEN** the selected region extends beyond display boundaries
- **THEN** the region is clipped to valid boundaries
- **AND** a warning is shown if significant clipping occurred
