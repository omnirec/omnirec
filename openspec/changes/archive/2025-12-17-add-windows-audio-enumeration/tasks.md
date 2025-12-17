## 1. Implementation

- [x] 1.1 Add WASAPI feature flags to `windows` crate dependency in `Cargo.toml` if needed
- [x] 1.2 Implement `list_audio_sources()` in `src-tauri/src/capture/windows/audio.rs`:
  - Initialize COM with `CoInitializeEx`
  - Create `IMMDeviceEnumerator` instance
  - Enumerate `eRender` (output) devices and map to `AudioSource` with `Output` type
  - Enumerate `eCapture` (input) devices and map to `AudioSource` with `Input` type
  - Extract device endpoint ID and friendly name for each device
  - Handle errors gracefully (return empty list on failure)
  - Clean up COM with `CoUninitialize`
- [x] 1.3 Update `init_audio_backend()` to succeed silently (enumeration-only, no persistent backend needed)

## 2. Testing

- [x] 2.1 Verify `list_audio_sources()` returns devices on Windows machine with audio hardware
- [x] 2.2 Verify output devices have `AudioSourceType::Output` and input devices have `AudioSourceType::Input`
- [x] 2.3 Verify device names are human-readable (friendly names)
- [x] 2.4 Verify application handles systems with no audio devices gracefully
- [x] 2.5 Verify application builds and runs on Linux (no regressions)

## 3. Validation

- [x] 3.1 Run `cargo clippy` with no warnings
- [x] 3.2 Run `cargo test` with all tests passing
- [x] 3.3 Manual test: Launch app on Windows and verify audio source dropdown shows devices
