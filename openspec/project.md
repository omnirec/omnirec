# Project Context

## Purpose

Screen Recorder is a high-performance desktop application for recording screens, windows, and custom regions. The core goals are:

- **Performance**: Minimal CPU/GPU overhead during recording
- **Simplicity**: Easy-to-use interface for quick captures
- **Privacy**: All processing local, no cloud dependencies
- **Flexibility**: Multiple output formats and capture modes
- **Free**: No subscriptions, accounts, or artificial limitations

## Tech Stack

- **Tauri v2**: Desktop application framework
- **Rust**: Backend for performance-critical recording operations
- **TypeScript**: Frontend logic (vanilla, no framework)
- **HTML/CSS**: UI markup and styling
- **Vite**: Build tooling and dev server
- **pnpm**: Package manager
- **FFmpeg**: Video encoding/transcoding (bundled or system)

## Project Conventions

### Code Style

**TypeScript/JavaScript:**
- Use TypeScript strict mode
- Prefer `const` over `let`; avoid `var`
- Use camelCase for variables/functions, PascalCase for types/classes
- Keep functions small and focused

**Rust:**
- Follow standard Rust conventions (rustfmt)
- Use `snake_case` for functions/variables, `PascalCase` for types
- Prefer `Result` for error handling over panics
- Document public APIs with doc comments

**CSS:**
- Use CSS custom properties for theming
- BEM-style naming for component classes
- Mobile-first responsive design

### Architecture Patterns

**Frontend-Backend Communication:**
- Use Tauri commands for synchronous operations
- Use Tauri events for async notifications (recording progress, errors)
- Keep frontend state minimal; backend is source of truth for recording state

**Recording Pipeline:**
- Capture → Encode → Write pattern
- Configurable encoder settings per output format
- Background thread for encoding to avoid UI blocking

**Error Handling:**
- Surface user-friendly error messages in UI
- Log detailed errors for debugging
- Graceful degradation when features unavailable

### Testing Strategy

- Unit tests for Rust recording logic
- Integration tests for Tauri commands
- Manual testing for UI and capture quality
- Test on Windows, macOS, and Linux

### Git Workflow

- `main` branch is stable
- Feature branches: `feature/<name>`
- Bug fixes: `fix/<description>`
- Conventional commits: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`

## Domain Context

**Screen Capture Concepts:**
- **Display**: Physical or virtual monitor
- **Window**: Application window that can be captured
- **Region**: User-defined rectangular area
- **Frame Rate**: Captures per second (typically 30 or 60 FPS)
- **Codec**: Video compression algorithm (H.264, VP9, etc.)
- **Container**: File format (MP4, WebM, MKV)

**Platform Considerations:**
- Windows: DXGI Desktop Duplication API, GDI fallback
- macOS: ScreenCaptureKit (macOS 12.3+), CGWindowListCreateImage fallback
- Linux: PipeWire, X11, Wayland considerations

## Important Constraints

- **No Cloud Dependencies**: Must work fully offline
- **Privacy First**: No telemetry, no external connections
- **Cross-Platform**: Must support Windows, macOS, and Linux
- **Performance**: Recording should not noticeably impact system performance
- **File Size**: Provide options for quality vs. file size tradeoffs

## External Dependencies

- **FFmpeg**: Video encoding (may be bundled or require system install)
- **System Screen Capture APIs**: Platform-specific capture mechanisms
- **Audio APIs**: Platform-specific audio capture for system/microphone audio
