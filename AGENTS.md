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

# Linux-only: Picker linting and tests
cd src-picker && cargo clippy --all-targets --all-features -- -D warnings
cd src-picker && cargo test --all-features
```

The CI workflow enforces these checks on all platforms. Code that passes locally but has linting warnings will fail in CI due to the `-D warnings` flag, which promotes all warnings to errors.

## Code Guidelines

### Cross-Platform

- Use conditional compilation (`#[cfg(...)]`) for platform-specific Rust code
- Abstract platform differences behind common interfaces

## Documentation and Notes

- Update the project `README.md` document any time significant features are added or changed.
