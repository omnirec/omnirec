<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->

# OmniRec - AI Assistant Guidelines

## Project Overview

This is a Tauri v2 desktop application for high-performance screen/window/region recording. The app prioritizes performance, privacy (no cloud), and ease of use.

## Key Directories

- `src/` - Frontend TypeScript, HTML, CSS
- `src-tauri/src/` - Rust backend code
- `src-tauri/Cargo.toml` - Rust dependencies
- `src-tauri/tauri.conf.json` - Tauri app configuration
- `openspec/` - Project specifications and proposals

## Development Commands

```bash
pnpm tauri dev         # Run development build
pnpm tauri build       # Production build
cargo test             # Run Rust tests (from src-tauri/)
cargo clippy           # Lint Rust code
pnpm build             # Build frontend only
```

## Code Guidelines

### Rust (Backend)

- Performance is critical; avoid unnecessary allocations in hot paths
- Use `Result<T, E>` for fallible operations; avoid `unwrap()` in production code
- Document Tauri commands with `///` doc comments
- Keep recording logic in dedicated modules, separate from Tauri bindings
- Use `tokio` for async operations where appropriate

### TypeScript (Frontend)

- Use strict TypeScript; no `any` types without justification
- Communicate with backend via `@tauri-apps/api` invoke/events
- Keep UI responsive; long operations should show progress
- Handle all error cases from backend commands

### Cross-Platform

- Test changes on Windows, macOS, and Linux when possible
- Use conditional compilation (`#[cfg(...)]`) for platform-specific Rust code
- Abstract platform differences behind common interfaces

## Performance Considerations

- Recording should use minimal CPU; prefer GPU acceleration where available
- Encoding happens on background threads
- Frontend updates during recording should be throttled
- Monitor memory usage; avoid frame buffer accumulation

## Privacy Requirements

- No network calls except user-initiated (e.g., checking for updates if user opts in)
- No telemetry or analytics
- All processing happens locally
- Recordings never leave the user's machine unless they explicitly share

## Testing

- Add unit tests for new Rust functions
- Test Tauri commands with integration tests
- Verify recording quality manually on all target platform

## Documentation and Notes

- Update the project `README.md` document any time significant features are added or changed.
