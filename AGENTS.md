# OmniRec - AI Assistant Guidelines

## Project Overview

This is a Tauri v2 desktop application for high-performance screen/window/region recording with support for voice transcription.

OmniRec uses VTX Engine - A voice processing and transcription engine written in Rust.
- The source code to VTX Engine is available in ~/ws/vtx-engine.
- Do not make changes to VTX Engine unless explicitly directed. When changes are necessary document what is needed only.

## Development Commands

```bash

# backend commands (run from the src-tauri directory)
cargo build
cargo test             # Run Rust tests (from src-tauri/)
cargo clippy           # Lint Rust code

# general commands
pnpm build             # Build frontend only

pnpm tauri build       # Production build (rarely needed)
```

Never run `pnpm tauri dev` - user will run this manually.

## Linting Requirements

**All code changes must pass strict linting before being committed.** Run these commands and fix any errors:

```bash
# TypeScript type checking
pnpm exec tsc --noEmit

# Rust linting (strict mode - warnings are errors)
cd src-tauri && cargo clippy --all-targets --all-features -- -D warnings

# Rust tests
cd src-tauri && cargo test --all-features

# Linux-only: Build picker (C++)
cmake -B src-picker/build -S src-picker -DCMAKE_BUILD_TYPE=Release
cmake --build src-picker/build
```

The CI workflow enforces these checks on all platforms. Code that passes locally but has linting warnings will fail in CI due to the `-D warnings` flag, which promotes all warnings to errors.

## Code Guidelines

### Cross-Platform

- Use conditional compilation (`#[cfg(...)]`) for platform-specific Rust code
- Abstract platform differences behind common interfaces

## Debugging

### Logs

- Logs are writen to ~/AppData/Local/omnirec/data/logs/

## Documentation and Notes

- Update the project `README.md` document any time significant features are added or changed.
- Maintain the `docs/cli.md` document to accurately and completely reflect the commands, options and arguments supported by the command-line interface.

