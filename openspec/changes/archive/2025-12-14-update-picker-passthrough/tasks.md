## 1. Implementation

- [x] 1.1 Add fallback picker binary path configuration (default: `hyprland-share-picker`)
- [x] 1.2 Modify IPC connection logic to detect when OmniRec is unavailable
- [x] 1.3 Implement standard picker execution with environment/argument passthrough
- [x] 1.4 Capture standard picker output and forward to stdout for XDPH
- [x] 1.5 Handle standard picker execution errors gracefully

## 2. Testing

- [ ] 2.1 Test that OmniRec recording works when app is running with selection
- [ ] 2.2 Test that other apps (e.g., OBS) can request capture when OmniRec is not running
- [ ] 2.3 Test fallback when OmniRec is running but has no selection
- [ ] 2.4 Verify standard picker UI appears correctly on fallback
