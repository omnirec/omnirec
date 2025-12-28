# Tasks: Client-Server Architecture Refactor

## 1. Prepare Workspace Structure

- [x] 1.1 Create `src-tauri/crates/omnirec-common/` crate for shared types
- [x] 1.2 Move shared types to common crate (WindowInfo, MonitorInfo, CaptureRegion, OutputFormat, RecordingState, etc.)
- [x] 1.3 Move IPC protocol types to common crate
- [x] 1.4 Update Cargo.toml to workspace configuration
- [x] 1.5 Verify existing code compiles with shared crate

## 2. Implement Service Binary

- [x] 2.1 Create `src-service/` directory (top-level, like src-picker)
- [x] 2.2 Add service binary target to Cargo.toml
- [x] 2.3 Implement IPC server (extend existing Linux IPC or create cross-platform version)
- [x] 2.4 Define socket paths per platform (Unix socket / Named pipe)
- [x] 2.5 Implement request routing and response handling

## 3. Migrate Capture Backends to Service

- [x] 3.1 Move `capture/` module to service crate
- [x] 3.2 Implement IPC handlers for window/monitor enumeration
- [x] 3.3 Implement IPC handlers for thumbnail capture
- [x] 3.4 Implement IPC handlers for highlight display
- [ ] 3.5 Verify capture functionality works via IPC

## 4. Migrate Encoder to Service

- [x] 4.1 Move `encoder/` module to service crate
- [x] 4.2 Move FFmpeg initialization to service startup
- [x] 4.3 Implement IPC handlers for output format get/set
- [x] 4.4 Verify encoding works via service

## 5. Migrate Recording State to Service

- [x] 5.1 Move RecordingManager to service crate
- [x] 5.2 Implement IPC handlers for recording state queries
- [x] 5.3 Implement IPC handlers for start/stop recording
- [x] 5.4 Implement event streaming for state changes
- [x] 5.5 Implement elapsed time streaming

## 6. Migrate Audio Capture to Service

- [x] 6.1 Move audio capture logic to service
- [x] 6.2 Implement IPC handlers for audio source enumeration
- [x] 6.3 Implement IPC handlers for audio configuration
- [x] 6.4 Verify audio capture works via service

## 7. Implement Client IPC Layer

- [x] 7.1 Create IPC client module in Tauri app
- [x] 7.2 Implement connection management (connect, reconnect, disconnect)
- [x] 7.3 Implement request/response handling
- [ ] 7.4 Implement event subscription and handling
- [x] 7.5 Add connection state to AppState

## 8. Update Tauri Commands

- [x] 8.1 Update capture commands to proxy via IPC (get_windows, get_monitors, thumbnails)
- [x] 8.2 Update recording commands to proxy via IPC (start, stop, state)
- [x] 8.3 Update audio commands to proxy via IPC
- [x] 8.4 Update platform detection commands (may remain local or proxy)
- [x] 8.5 Update highlight commands to proxy via IPC
- [x] 8.6 Update format commands to proxy via IPC

## 9. Implement Service Lifecycle Management

- [x] 9.1 Implement service startup from client (spawn process)
- [x] 9.2 Implement service readiness detection (wait for socket)
- [x] 9.3 Implement graceful shutdown handling
- [x] 9.4 Add service crash detection and error handling
- [ ] 9.5 Test service startup on all platforms

## 10. Integrate Picker with Service

- [x] 10.1 Update picker to use unified IPC interface
- [x] 10.2 Ensure picker still works for selection queries
- [x] 10.3 Ensure token validation/storage still works
- [ ] 10.4 Test picker fallback behavior

## 11. Update Build and Packaging

- [x] 11.1 Update CI workflow to build both binaries
- [x] 11.2 Update release workflow for service binary
- [x] 11.3 Update AUR PKGBUILD for new binary
- [x] 11.4 Service binary discovery works in dev/production

## 12. Testing and Validation

- [ ] 12.1 Test window recording via client-service
- [ ] 12.2 Test display recording via client-service
- [ ] 12.3 Test region recording via client-service
- [ ] 12.4 Test GNOME portal recording via client-service
- [ ] 12.5 Test audio recording via client-service
- [ ] 12.6 Test transcoding via client-service
- [ ] 12.7 Test on Windows
- [ ] 12.8 Test on Linux (Hyprland)
- [ ] 12.9 Test on Linux (GNOME)
- [ ] 12.10 Test on macOS
- [x] 12.11 Run clippy and fix warnings
- [x] 12.12 Run tests

## 13. Security Implementation (P0 - MVP)

- [x] 13.1 Implement peer credential verification (Linux: SO_PEERCRED)
- [x] 13.2 Implement peer credential verification (macOS: LOCAL_PEERCRED)
- [x] 13.3 Implement peer credential verification (Windows: GetNamedPipeClientProcessId)
- [x] 13.4 Verify connecting process executable path matches trusted locations
- [x] 13.5 Set explicit socket file permissions (0600) after creation
- [x] 13.6 Set socket parent directory permissions (0700)
- [ ] 13.7 Implement Windows named pipe security descriptors (current user only)
- [x] 13.8 Implement maximum message size limit (64KB)
- [x] 13.9 Add input validation for numeric ranges (dimensions, coordinates)
- [x] 13.10 Add input validation for string field lengths
- [x] 13.11 Validate identifiers are not paths (monitor_id, source_id patterns)
- [x] 13.12 Canonicalize and validate output directory paths
- [ ] 13.13 Sanitize error messages (no file paths to clients)

## 14. Security Implementation (P1)

- [ ] 14.1 Implement per-client rate limiting (100 req/sec max)
- [ ] 14.2 Implement maximum concurrent connection limit (5 clients)
- [ ] 14.3 Add read timeout on client connections (30 seconds)
- [ ] 14.4 Implement event stream backpressure (max 100 pending events)
- [ ] 14.5 Verify approval token uses CSPRNG for generation
- [ ] 14.6 Add detailed server-side logging for security events

## 15. Security Testing

- [ ] 15.1 Test peer credential verification rejects unauthorized processes
- [ ] 15.2 Test message size limit enforcement
- [ ] 15.3 Test path traversal prevention
- [ ] 15.4 Test rate limiting under load
- [ ] 15.5 Test socket/file permissions on each platform
- [ ] 15.6 Run cargo audit for dependency vulnerabilities
- [ ] 15.7 Test with malformed JSON input
- [ ] 15.8 Test with oversized numeric values

## 16. Documentation

- [ ] 16.1 Update README with new architecture overview
- [ ] 16.2 Document IPC protocol (for future clients)
- [ ] 16.3 Update AGENTS.md with new module structure
- [ ] 16.4 Document security model and trust boundaries

## Dependencies

- Tasks 3-6 depend on Task 2 (service binary exists)
- Tasks 7-8 depend on Tasks 3-6 (service APIs exist)
- Task 9 can be done in parallel with Tasks 3-6
- Task 10 depends on Tasks 7-8 (client IPC works)
- Task 11 depends on Tasks 1-10 (both binaries functional)
- Task 12 depends on Task 11 (packaging complete)
- **Task 13 (Security P0) should be integrated into Task 2** (security from the start)
- Task 14 (Security P1) can be done after MVP functionality works
- Task 15 (Security Testing) depends on Tasks 13-14

## Parallelizable Work

- Tasks 3, 4, 5, 6 can be done in parallel once Task 2 is complete
- Task 9 can be done in parallel with Tasks 3-6
- Tasks 12.7, 12.8, 12.9, 12.10 can be done in parallel
- Task 13 should be done alongside Task 2 (not after)
- Tasks 15.1-15.8 can be done in parallel
