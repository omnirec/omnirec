## Context

macOS requires platform-specific APIs for screen capture:
- **ScreenCaptureKit (SCK)**: Apple's modern capture framework (macOS 12.3+), provides GPU-accelerated capture
- **Core Graphics (CG)**: Lower-level API for window/display enumeration, CGWindowListCreateImage as fallback
- **TCC (Transparency, Consent, and Control)**: macOS permission system for screen recording authorization

The existing platform abstraction traits (`CaptureBackend`, `WindowEnumerator`, `MonitorEnumerator`, `HighlightProvider`) define the interface. The macOS implementation must conform to these traits while handling platform-specific details internally.

## Goals / Non-Goals

**Goals:**
- Implement full macOS capture support matching Windows/Linux functionality
- Support window, display, and region capture modes
- Handle macOS screen recording permissions gracefully
- Maintain high performance using ScreenCaptureKit
- Keep all changes isolated to macOS platform code

**Non-Goals:**
- Support macOS versions prior to 12.3 (ScreenCaptureKit minimum)
- Implement audio capture (future change)
- Change the platform abstraction traits
- Modify Windows or Linux implementations

## Decisions

### Decision 1: Use ScreenCaptureKit as Primary Capture API
**What**: Use SCK for all frame capture operations
**Why**: 
- GPU-accelerated with minimal CPU overhead
- Provides direct access to window/display content
- Apple's recommended API for screen recording apps
- Supports cursor capture and high frame rates
**Alternatives considered**:
- CGWindowListCreateImage: Older API, higher CPU usage, no streaming
- AVFoundation screen capture: Deprecated in favor of SCK

### Decision 2: Use Rust ScreenCaptureKit Bindings
**What**: Use the `screencapturekit` crate for Rust bindings
**Why**:
- Mature Rust bindings with safe abstractions
- Active maintenance and community support
- Covers SCStreamConfiguration, SCContentFilter, SCStream APIs
**Alternatives considered**:
- Raw Objective-C FFI: More complex, error-prone, maintenance burden
- `objc` crate direct bindings: Requires more boilerplate

### Decision 3: Use Core Graphics for Enumeration
**What**: Use `core-graphics` crate for window/display listing
**Why**:
- Stable API available on all macOS versions
- CGWindowListCopyWindowInfo provides comprehensive window metadata
- CGDisplayCopyAllDisplayModes for display enumeration
- Lighter weight than SCK for enumeration-only operations
**Alternatives considered**:
- SCK's SCShareableContent: Requires async enumeration, triggers permission prompts

### Decision 4: Module Structure Matching Windows/Linux
**What**: Create `mod.rs`, `window_list.rs`, `monitor_list.rs`, `recorder.rs`, `region.rs`, `highlight.rs`
**Why**:
- Consistent codebase structure across platforms
- Easier to navigate and maintain
- Clear separation of concerns

### Decision 5: Permission Handling Strategy
**What**: Check permissions at capture start, prompt user if needed, fail gracefully with clear error
**Why**:
- macOS requires explicit screen recording permission
- Users must grant permission through System Preferences
- Cannot programmatically request permission (only trigger the prompt)
**Implementation**:
- Use `CGPreflightScreenCaptureAccess()` to check status
- Use `CGRequestScreenCaptureAccess()` to trigger prompt
- Return `CaptureError::PermissionDenied` with actionable message

## Risks / Trade-offs

### Risk 1: macOS 12.3+ Requirement
**Risk**: Users on older macOS versions cannot use the app
**Mitigation**: 
- Document minimum version requirement clearly
- Show user-friendly error on older macOS versions
- Consider CGWindowListCreateImage fallback for basic capture (future enhancement)

### Risk 2: ScreenCaptureKit Crate Stability
**Risk**: Third-party crate may have bugs or become unmaintained
**Mitigation**:
- Pin to stable version
- Have fallback plan to fork/maintain if needed
- Keep capture code modular for easy replacement

### Risk 3: Permission UX Friction
**Risk**: Users may be confused by permission prompts
**Mitigation**:
- Provide clear UI messaging before triggering permission prompt
- Detect permission denied state and guide users to System Preferences
- Test permission flow thoroughly across macOS versions

## Migration Plan

No migration needed - this is a new implementation. The existing stub will be replaced.

**Rollback**: Revert to stub implementation if critical issues discovered.

## Open Questions

1. **Highlight implementation**: Should we use NSWindow for highlight overlay or a Tauri webview window? (Decision: Start with NSWindow for consistency with platform patterns)

2. **Retina display handling**: ScreenCaptureKit handles scaling automatically, but should we offer options for capture resolution? (Decision: Use native resolution by default, defer quality options to future change)
