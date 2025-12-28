# ipc-interface Specification

## Purpose
TBD - created by archiving change refactor-client-server-architecture. Update Purpose after archive.
## Requirements
### Requirement: IPC Protocol Format

The system SHALL use a JSON-based protocol with length-prefixed framing for IPC communication.

#### Scenario: Message framing

- **WHEN** a message is sent over IPC
- **THEN** it is prefixed with a 4-byte little-endian length
- **AND** the payload is valid JSON
- **AND** the receiver can reliably parse multiple messages from the stream

#### Scenario: Request format

- **WHEN** a client sends a request
- **THEN** the JSON includes a `type` field identifying the operation
- **AND** additional fields contain operation-specific parameters

#### Scenario: Response format

- **WHEN** the service responds to a request
- **THEN** the JSON includes a `type` field indicating the response type
- **AND** successful responses include operation-specific data
- **AND** error responses include a `type: "error"` and `message` field

### Requirement: Enumeration Operations

The IPC interface SHALL support querying available capture sources.

#### Scenario: List windows via IPC

- **WHEN** the client sends `{"type": "list_windows"}`
- **THEN** the service responds with `{"type": "windows", "windows": [...]}`
- **AND** each window object includes handle, title, and process_name

#### Scenario: List monitors via IPC

- **WHEN** the client sends `{"type": "list_monitors"}`
- **THEN** the service responds with `{"type": "monitors", "monitors": [...]}`
- **AND** each monitor object includes id, name, width, height, x, y, and is_primary

#### Scenario: List audio sources via IPC

- **WHEN** the client sends `{"type": "list_audio_sources"}`
- **THEN** the service responds with `{"type": "audio_sources", "sources": [...]}`
- **AND** each source object includes id, name, and source_type

### Requirement: Recording Control Operations

The IPC interface SHALL support starting and stopping recordings.

#### Scenario: Start window recording via IPC

- **WHEN** the client sends `{"type": "start_window_capture", "window_handle": <handle>}`
- **THEN** the service starts capturing the specified window
- **AND** responds with `{"type": "recording_started"}` on success
- **OR** responds with `{"type": "error", "message": "..."}` on failure

#### Scenario: Start display recording via IPC

- **WHEN** the client sends `{"type": "start_display_capture", "monitor_id": "...", "width": ..., "height": ...}`
- **THEN** the service starts capturing the specified display
- **AND** responds with `{"type": "recording_started"}` on success

#### Scenario: Start region recording via IPC

- **WHEN** the client sends `{"type": "start_region_capture", "monitor_id": "...", "x": ..., "y": ..., "width": ..., "height": ...}`
- **THEN** the service starts capturing the specified region
- **AND** responds with `{"type": "recording_started"}` on success

#### Scenario: Start portal recording via IPC

- **WHEN** the client sends `{"type": "start_portal_capture"}`
- **THEN** the service initiates portal-based capture (GNOME mode)
- **AND** responds with `{"type": "recording_started"}` on success

#### Scenario: Stop recording via IPC

- **WHEN** the client sends `{"type": "stop_recording"}`
- **THEN** the service stops the active recording
- **AND** finalizes the video file
- **AND** performs transcoding if needed
- **AND** responds with `{"type": "recording_stopped", "file_path": "...", "source_path": "..."}`

### Requirement: State Query Operations

The IPC interface SHALL support querying recording state.

#### Scenario: Get recording state via IPC

- **WHEN** the client sends `{"type": "get_recording_state"}`
- **THEN** the service responds with `{"type": "recording_state", "state": "idle"|"recording"|"saving"}`

#### Scenario: Get elapsed time via IPC

- **WHEN** the client sends `{"type": "get_elapsed_time"}`
- **THEN** the service responds with `{"type": "elapsed_time", "seconds": <number>}`

### Requirement: Configuration Operations

The IPC interface SHALL support output format and audio configuration.

#### Scenario: Get output format via IPC

- **WHEN** the client sends `{"type": "get_output_format"}`
- **THEN** the service responds with `{"type": "output_format", "format": "mp4"|"webm"|...}`

#### Scenario: Set output format via IPC

- **WHEN** the client sends `{"type": "set_output_format", "format": "webm"}`
- **THEN** the service updates the output format
- **AND** responds with `{"type": "ok"}` on success
- **OR** responds with `{"type": "error", "message": "..."}` if recording is in progress

#### Scenario: Get audio config via IPC

- **WHEN** the client sends `{"type": "get_audio_config"}`
- **THEN** the service responds with `{"type": "audio_config", "enabled": ..., "source_id": ..., "microphone_id": ..., "echo_cancellation": ...}`

#### Scenario: Set audio config via IPC

- **WHEN** the client sends `{"type": "set_audio_config", "enabled": true, "source_id": "...", ...}`
- **THEN** the service updates the audio configuration
- **AND** responds with `{"type": "ok"}` on success

### Requirement: Thumbnail Operations

The IPC interface SHALL support capturing thumbnails and previews.

#### Scenario: Get window thumbnail via IPC

- **WHEN** the client sends `{"type": "get_window_thumbnail", "window_handle": <handle>}`
- **THEN** the service captures a thumbnail of the window
- **AND** responds with `{"type": "thumbnail", "data": "<base64>", "width": ..., "height": ...}`

#### Scenario: Get display thumbnail via IPC

- **WHEN** the client sends `{"type": "get_display_thumbnail", "monitor_id": "..."}`
- **THEN** the service captures a thumbnail of the display
- **AND** responds with `{"type": "thumbnail", "data": "<base64>", "width": ..., "height": ...}`

#### Scenario: Get region preview via IPC

- **WHEN** the client sends `{"type": "get_region_preview", "monitor_id": "...", "x": ..., "y": ..., "width": ..., "height": ...}`
- **THEN** the service captures a preview of the region
- **AND** responds with `{"type": "thumbnail", "data": "<base64>", "width": ..., "height": ...}`

### Requirement: Highlight Operations

The IPC interface SHALL support displaying visual highlights.

#### Scenario: Show display highlight via IPC

- **WHEN** the client sends `{"type": "show_display_highlight", "x": ..., "y": ..., "width": ..., "height": ...}`
- **THEN** the service displays a highlight border at the specified location
- **AND** responds with `{"type": "ok"}`

#### Scenario: Show window highlight via IPC

- **WHEN** the client sends `{"type": "show_window_highlight", "window_handle": <handle>}`
- **THEN** the service displays a highlight border around the window
- **AND** responds with `{"type": "ok"}`

### Requirement: Event Streaming

The IPC interface SHALL support subscribing to real-time events.

#### Scenario: Subscribe to events

- **WHEN** the client sends `{"type": "subscribe_events"}`
- **THEN** the service begins streaming events to that client
- **AND** responds with `{"type": "subscribed"}`

#### Scenario: State change event

- **WHEN** the recording state changes
- **AND** a client is subscribed to events
- **THEN** the service sends `{"type": "event", "event": "state_changed", "state": "recording"}`

#### Scenario: Elapsed time event

- **WHEN** a recording is in progress
- **AND** a client is subscribed to events
- **THEN** the service periodically sends `{"type": "event", "event": "elapsed_time", "seconds": ...}`

#### Scenario: Transcoding events

- **WHEN** transcoding starts or completes
- **AND** a client is subscribed to events
- **THEN** the service sends `{"type": "event", "event": "transcoding_started", "format": "..."}`
- **AND** the service sends `{"type": "event", "event": "transcoding_complete", "success": true, "path": "..."}`

### Requirement: Picker Compatibility

The IPC interface SHALL maintain compatibility with the existing picker protocol.

#### Scenario: Query selection via IPC

- **WHEN** the picker sends `{"type": "query_selection"}`
- **THEN** the service responds with the current selection
- **OR** responds with `{"type": "no_selection"}` if none is set

#### Scenario: Validate token via IPC

- **WHEN** the picker sends `{"type": "validate_token", "token": "..."}`
- **THEN** the service validates the token against stored value
- **AND** responds with `{"type": "token_valid"}` or `{"type": "token_invalid"}`

#### Scenario: Store token via IPC

- **WHEN** the picker sends `{"type": "store_token", "token": "..."}`
- **THEN** the service stores the token
- **AND** responds with `{"type": "token_stored"}`

