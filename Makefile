# OmniRec Makefile
# Build all components for testing

# Ensure bash shell for cross-platform targets (Windows CI uses Git Bash)
ifeq ($(OS),Windows_NT)
SHELL := bash
else
SHELL := /usr/bin/env bash
endif

.PHONY: all clean build build-debug build-release build-cuda \
        frontend client client-debug client-release cli cli-debug cli-release picker \
        package stage-cli stage-ffmpeg stub-sidecar \
        run-cli run-cli-release \
        lint lint-rust lint-rust-common lint-rust-tauri lint-rust-cli lint-ts test \
        install-deps check-binaries help

# Optional args to pass through to pnpm tauri build (e.g. ARGS="--target aarch64-apple-darwin")
ARGS ?=

# Default target
all: build

# Build all components in release mode
build: build-release

# Build all components in debug mode (faster compilation)
build-debug: frontend client-debug cli-debug picker

# Build all components in release mode
build-release: frontend client-release cli-release picker

# Build all components with CUDA acceleration (Linux only - Windows always includes CUDA)
build-cuda: frontend client-cuda cli-release picker

# =============================================================================
# Individual Components
# =============================================================================

# Build frontend (TypeScript/Vite)
frontend:
	@echo "==> Building frontend..."
	pnpm build

# Build omnirec Tauri app (debug)
client-debug:
	@echo "==> Building omnirec-app (debug)..."
	cd src-tauri && cargo build

# Build omnirec Tauri app (release)
client-release:
	@echo "==> Building omnirec-app (release)..."
	cd src-tauri && cargo build --release

# Build omnirec Tauri app with CUDA acceleration (release, Linux only)
# Requires: NVIDIA CUDA Toolkit (nvcc, cuBLAS) on Linux
# Windows always includes CUDA binaries - use 'client-release' instead
# macOS: No effect (Metal acceleration is always used)
client-cuda:
	@echo "==> Building omnirec-app with CUDA (release)..."
	cd src-tauri && cargo build --release --features cuda

# Alias for release
client: client-release

# =============================================================================
# Packaging
# =============================================================================

# Build and stage the CLI sidecar binary into src-tauri/binaries/ for Tauri bundling.
# Required before running pnpm tauri build or make package.
# Supports optional ARGS for cross-compilation (e.g. make stage-cli ARGS="--target aarch64-apple-darwin")
stage-cli:
	@echo "==> Building CLI binary..."
	@if echo "$(ARGS)" | grep -q "aarch64-apple-darwin"; then \
		cargo build --release -p omnirec-cli --target aarch64-apple-darwin; \
	elif echo "$(ARGS)" | grep -q "x86_64-apple-darwin"; then \
		cargo build --release -p omnirec-cli --target x86_64-apple-darwin; \
	else \
		cargo build --release -p omnirec-cli; \
	fi
	@echo "==> Staging CLI sidecar binary into src-tauri/binaries/..."
	@mkdir -p src-tauri/binaries
	@if echo "$(ARGS)" | grep -q "aarch64-apple-darwin"; then \
		cp target/aarch64-apple-darwin/release/omnirec src-tauri/binaries/omnirec-cli-aarch64-apple-darwin; \
	elif echo "$(ARGS)" | grep -q "x86_64-apple-darwin"; then \
		cp target/x86_64-apple-darwin/release/omnirec src-tauri/binaries/omnirec-cli-x86_64-apple-darwin; \
	elif [ "$$(uname -s)" = "Linux" ]; then \
		cp target/release/omnirec src-tauri/binaries/omnirec-cli-x86_64-unknown-linux-gnu; \
	elif [ "$$(uname -s)" = "Darwin" ]; then \
		cp target/release/omnirec src-tauri/binaries/omnirec-cli-$$(rustc -vV | grep host | cut -d' ' -f2); \
	else \
		cp target/release/omnirec.exe "src-tauri/binaries/omnirec-cli-$$(rustc -vV | grep host | cut -d' ' -f2).exe"; \
	fi

# Download and stage the FFmpeg binary into src-tauri/binaries/ for Tauri bundling.
# On Windows/macOS, FFmpeg is bundled with the app. On Linux, it's a system dependency.
# Supports optional ARGS for cross-compilation (e.g. make stage-ffmpeg ARGS="--target aarch64-apple-darwin")
FFMPEG_WIN_URL := https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip
FFMPEG_MAC_X64_URL := https://evermeet.cx/ffmpeg/getrelease/zip
FFMPEG_MAC_ARM_URL := https://www.osxexperts.net/ffmpeg80arm.zip

stage-ffmpeg:
	@mkdir -p src-tauri/binaries
	@if echo "$(ARGS)" | grep -q "aarch64-apple-darwin"; then \
		echo "==> Downloading FFmpeg for macOS ARM64..."; \
		curl -L -o /tmp/ffmpeg-mac-arm.zip "$(FFMPEG_MAC_ARM_URL)"; \
		unzip -o /tmp/ffmpeg-mac-arm.zip -d /tmp/ffmpeg-mac-arm; \
		cp /tmp/ffmpeg-mac-arm/ffmpeg src-tauri/binaries/ffmpeg-aarch64-apple-darwin; \
		chmod +x src-tauri/binaries/ffmpeg-aarch64-apple-darwin; \
		rm -rf /tmp/ffmpeg-mac-arm /tmp/ffmpeg-mac-arm.zip; \
	elif echo "$(ARGS)" | grep -q "x86_64-apple-darwin"; then \
		echo "==> Downloading FFmpeg for macOS x86_64..."; \
		curl -L -o /tmp/ffmpeg-mac-x64.zip "$(FFMPEG_MAC_X64_URL)"; \
		unzip -o /tmp/ffmpeg-mac-x64.zip -d /tmp/ffmpeg-mac-x64; \
		cp /tmp/ffmpeg-mac-x64/ffmpeg src-tauri/binaries/ffmpeg-x86_64-apple-darwin; \
		chmod +x src-tauri/binaries/ffmpeg-x86_64-apple-darwin; \
		rm -rf /tmp/ffmpeg-mac-x64 /tmp/ffmpeg-mac-x64.zip; \
	elif [ "$$(uname -s)" = "Darwin" ]; then \
		echo "==> Downloading FFmpeg for macOS (host)..."; \
		TRIPLE=$$(rustc -vV | grep host | cut -d' ' -f2); \
		curl -L -o /tmp/ffmpeg-mac.zip "$(FFMPEG_MAC_X64_URL)"; \
		unzip -o /tmp/ffmpeg-mac.zip -d /tmp/ffmpeg-mac; \
		cp /tmp/ffmpeg-mac/ffmpeg "src-tauri/binaries/ffmpeg-$$TRIPLE"; \
		chmod +x "src-tauri/binaries/ffmpeg-$$TRIPLE"; \
		rm -rf /tmp/ffmpeg-mac /tmp/ffmpeg-mac.zip; \
	elif [ "$$(uname -s)" = "Linux" ]; then \
		echo "==> Skipping FFmpeg staging on Linux (uses system package)"; \
	else \
		echo "==> Downloading FFmpeg for Windows..."; \
		TRIPLE=$$(rustc -vV | grep host | cut -d' ' -f2); \
		curl -L -o /tmp/ffmpeg-win.zip "$(FFMPEG_WIN_URL)"; \
		unzip -o /tmp/ffmpeg-win.zip "*/bin/ffmpeg.exe" -d /tmp/ffmpeg-win; \
		cp /tmp/ffmpeg-win/*/bin/ffmpeg.exe "src-tauri/binaries/ffmpeg-$$TRIPLE.exe"; \
		rm -rf /tmp/ffmpeg-win /tmp/ffmpeg-win.zip; \
	fi

# Build the complete installer package (CLI sidecar + FFmpeg + Tauri app).
# Supports optional ARGS for cross-compilation (e.g. make package ARGS="--target aarch64-apple-darwin")
package: stage-cli stage-ffmpeg
	@echo "==> Building Tauri installer package..."
	pnpm tauri build $(ARGS)

# Build omnirec CLI (debug)
cli-debug:
	@echo "==> Building omnirec CLI (debug)..."
	cd src-cli && cargo build

# Build omnirec CLI (release)
cli-release:
	@echo "==> Building omnirec CLI (release)..."
	cd src-cli && cargo build --release

# Alias for release
cli: cli-release

# Build omnirec-picker (C++/Qt6)
picker:
	@echo "==> Building omnirec-picker..."
	cmake -B src-picker/build -S src-picker -DCMAKE_BUILD_TYPE=Release
	cmake --build src-picker/build

# =============================================================================
# Linting
# =============================================================================

# Run all linters
lint: lint-rust-common lint-rust-tauri lint-rust-cli lint-ts

# Rust linting (all crates)
lint-rust: lint-rust-common lint-rust-tauri lint-rust-cli

# Rust linting - common library
lint-rust-common:
	@echo "==> Linting src-common..."
	cargo clippy -p omnirec-common --all-targets --all-features -- -D warnings

# Create empty stub sidecar binaries for the current host triple so tauri-build
# validation passes during lint/clippy without requiring real pre-built binaries.
# Only creates stubs if no binary (real or stub) already exists for this triple.
stub-sidecar:
	@mkdir -p src-tauri/binaries
	@TRIPLE=$$(rustc -vV | grep '^host:' | cut -d' ' -f2); \
	EXT=""; \
	if echo "$$TRIPLE" | grep -q "windows"; then EXT=".exe"; fi; \
	STUB=src-tauri/binaries/omnirec-cli-$$TRIPLE$$EXT; \
	if [ ! -f "$$STUB" ]; then \
		echo "==> Creating sidecar stub: $$STUB"; \
		touch "$$STUB"; \
	fi; \
	FFMPEG_STUB=src-tauri/binaries/ffmpeg-$$TRIPLE$$EXT; \
	if [ ! -f "$$FFMPEG_STUB" ]; then \
		echo "==> Creating FFmpeg stub: $$FFMPEG_STUB"; \
		touch "$$FFMPEG_STUB"; \
	fi

# Rust linting - main app
lint-rust-tauri: stub-sidecar
	@echo "==> Linting src-tauri..."
	# Note: --all-features omitted because 'cuda' feature requires NVIDIA CUDA Toolkit (Linux only)
	# Windows always uses CUDA-enabled prebuilt binaries regardless of features
	cargo clippy -p omnirec --all-targets -- -D warnings

# Rust linting - CLI
lint-rust-cli:
	@echo "==> Linting src-cli..."
	cargo clippy -p omnirec-cli --all-targets --all-features -- -D warnings

# TypeScript linting
lint-ts:
	@echo "==> TypeScript type check..."
	pnpm exec tsc --noEmit

# =============================================================================
# Testing
# =============================================================================

# Run all tests
test: test-rust

# Rust tests (all crates)
test-rust:
	@echo "==> Testing src-common..."
	cd src-common && cargo test --all-features
	@echo "==> Testing src-tauri..."
	cd src-tauri && cargo test --all-features
	@echo "==> Testing src-cli..."
	cd src-cli && cargo test --all-features

# =============================================================================
# Cleaning
# =============================================================================

# Clean all build artifacts
clean:
	@echo "==> Cleaning frontend..."
	rm -rf dist
	@echo "==> Cleaning Rust targets..."
	cd src-common && cargo clean
	cd src-tauri && cargo clean
	cd src-cli && cargo clean
	@echo "==> Cleaning picker..."
	rm -rf src-picker/build

# Clean only Rust debug builds (keeps release)
clean-debug:
	cd src-common && cargo clean --profile dev
	cd src-tauri && cargo clean --profile dev
	cd src-cli && cargo clean --profile dev

# =============================================================================
# Dependencies
# =============================================================================

# Install all dependencies
install-deps:
	@echo "==> Installing pnpm dependencies..."
	pnpm install
	@echo "==> Checking Rust toolchain..."
	rustup show
	@echo ""
	@echo "Note: System dependencies must be installed manually."
	@echo "See README.md for platform-specific instructions."

# =============================================================================
# Development Helpers
# =============================================================================

# Build and run CLI (debug)
run-cli: cli-debug
	@echo "==> Running omnirec CLI (debug)..."
	./target/debug/omnirec

# Build and run CLI (release)
run-cli-release: cli-release
	@echo "==> Running omnirec CLI (release)..."
	./target/release/omnirec

# Check if all binaries exist (after build)
check-binaries:
	@echo "Checking built binaries..."
	@test -f src-tauri/target/release/omnirec && echo "  [OK] omnirec (Tauri app)" || echo "  [MISSING] omnirec (Tauri app)"
	@test -f target/release/omnirec && echo "  [OK] omnirec (CLI)" || echo "  [MISSING] omnirec (CLI)"
	@test -f src-picker/build/omnirec-picker && echo "  [OK] omnirec-picker" || echo "  [MISSING] omnirec-picker (Linux only)"

# =============================================================================
# Help
# =============================================================================

help:
	@echo "OmniRec Build System"
	@echo ""
	@echo "Usage: make [target]"
	@echo ""
	@echo "Build Targets:"
	@echo "  all, build       Build all components (release mode)"
	@echo "  build-debug      Build all components (debug mode, faster)"
	@echo "  build-release    Build all components (release mode)"
	@echo "  build-cuda       Build with CUDA GPU acceleration (Linux only)"
	@echo "  frontend         Build frontend only"
	@echo "  client           Build omnirec Tauri app (release)"
	@echo "  client-debug     Build omnirec Tauri app (debug)"
	@echo "  client-cuda      Build omnirec Tauri app with CUDA (Linux only)"
	@echo "  cli              Build omnirec CLI (release)"
	@echo "  cli-debug        Build omnirec CLI (debug)"
	@echo "  picker           Build omnirec-picker (C++/Qt6)"
	@echo ""
	@echo "Package Targets:"
	@echo "  package          Build complete installer with CLI + FFmpeg sidecars (uses pnpm tauri build)"
	@echo "                   Supports ARGS for cross-compilation:"
	@echo "                   make package ARGS=\"--target aarch64-apple-darwin\""
	@echo "  stage-cli        Build CLI binary and stage into src-tauri/binaries/"
	@echo "                   (run this before pnpm tauri build when using tauri-action)"
	@echo "  stage-ffmpeg     Download FFmpeg binary and stage into src-tauri/binaries/"
	@echo "                   (Windows/macOS only; Linux uses system FFmpeg)"
	@echo "  stub-sidecar     Create empty sidecar stubs for host triple (used by lint)"
	@echo ""
	@echo "Quality Targets:"
	@echo "  lint             Run all linters"
	@echo "  lint-rust        Run Rust clippy on all crates"
	@echo "  lint-rust-common Run Rust clippy on src-common"
	@echo "  lint-rust-tauri  Run Rust clippy on src-tauri"
	@echo "  lint-rust-cli    Run Rust clippy on src-cli"
	@echo "  lint-ts          Run TypeScript type check"
	@echo "  test             Run all tests"
	@echo "  test-rust        Run Rust tests on all crates"
	@echo ""
	@echo "Run Targets:"
	@echo "  run-cli             Build and run CLI (debug)"
	@echo "  run-cli-release     Build and run CLI (release)"
	@echo ""
	@echo "Utility Targets:"
	@echo "  clean            Clean all build artifacts"
	@echo "  clean-debug      Clean only debug builds"
	@echo "  install-deps     Install npm/pnpm dependencies"
	@echo "  check-binaries   Check if all binaries were built"
	@echo "  help             Show this help message"
	@echo ""
	@echo "CUDA Acceleration:"
	@echo "  - Windows: CUDA binaries always included; falls back to CPU if no GPU"
	@echo "  - Linux: Use 'cuda' targets to enable GPU acceleration"
	@echo "           Requires NVIDIA CUDA Toolkit (nvcc, cuBLAS) at build time"
	@echo "  - macOS: No effect (Metal acceleration is always used)"
