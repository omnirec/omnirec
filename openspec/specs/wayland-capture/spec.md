# wayland-capture Specification

## Purpose
TBD - created by archiving change add-wayland-linux-support. Update Purpose after archive.
## Requirements
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

The system SHALL capture screen regions by capturing the full monitor via PipeWire and cropping frames to the selected region.

#### Scenario: Capture region from display stream

- **WHEN** recording starts in region mode
- **THEN** a full display stream is obtained via portal for the region's monitor
- **AND** each frame is cropped to the selected region coordinates (monitor-relative)
- **AND** output dimensions match the region size
- **AND** cropped frames are delivered at the configured frame rate

#### Scenario: Region extends beyond display

- **WHEN** the selected region extends beyond display boundaries
- **THEN** the region is clipped to valid boundaries during capture setup
- **AND** a warning is shown if significant clipping occurred
- **AND** recording proceeds with the clipped region

#### Scenario: Region validation before capture

- **WHEN** `start_region_capture()` is called
- **THEN** the backend validates region coordinates against monitor dimensions
- **AND** returns `CaptureError::InvalidRegion` if region is invalid
- **AND** the error message indicates the specific validation failure

#### Scenario: Monitor resolution changes during region recording

- **WHEN** the monitor resolution changes during region recording
- **THEN** the PipeWire stream ends
- **AND** recording stops gracefully
- **AND** partial recording is saved with original region dimensions
- **AND** the user is notified that recording stopped due to monitor change

### Requirement: Frame Cropping Performance

The system SHALL crop frames efficiently with minimal CPU overhead to avoid impacting recording quality.

#### Scenario: CPU overhead within acceptable limits

- **WHEN** recording a region from a full monitor stream
- **THEN** frame cropping adds no more than 5% CPU overhead compared to full monitor capture
- **AND** frame delivery rate maintains the configured FPS

#### Scenario: Memory-efficient cropping

- **WHEN** cropping frames
- **THEN** only the cropped region is copied to the output frame buffer
- **AND** full monitor frame buffers are released after cropping
- **AND** memory usage scales with region size, not full monitor size

### Requirement: Linux Visual Highlight

The system SHALL display a visual highlight border on Linux/Hyprland when previewing capture targets.

#### Scenario: Show window highlight on Linux

- **GIVEN** a window is selected for capture on Linux/Hyprland
- **WHEN** `show_highlight` is called with the window's bounds
- **THEN** a visible border is rendered around the window
- **AND** the highlight is non-interactive (click-through)
- **AND** the highlight auto-dismisses after approximately 800ms

#### Scenario: Show display highlight on Linux

- **GIVEN** a display is selected for capture on Linux/Hyprland
- **WHEN** `show_highlight` is called with the display's bounds
- **THEN** a visible border is rendered around the display edges
- **AND** the highlight is visible above all other windows
- **AND** the highlight auto-dismisses after approximately 800ms

#### Scenario: Highlight visual consistency

- **GIVEN** the highlight is displayed on Linux
- **THEN** the border color is #2196F3 (blue)
- **AND** the border width is approximately 6-8 pixels
- **AND** the appearance is consistent with Windows and macOS highlights

#### Scenario: Rapid successive highlights

- **GIVEN** a highlight is currently visible on Linux
- **WHEN** `show_highlight` is called again with different coordinates
- **THEN** the previous highlight is dismissed
- **AND** a new highlight is shown at the new location

