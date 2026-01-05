# OmniRec Makefile
# Build all components for testing

.PHONY: all clean build build-debug build-release build-cuda \
        frontend service service-cuda client cli picker \
        run-service run-service-release run-service-cuda \
        run-cli run-cli-release \
        lint lint-rust lint-ts test \
        install-deps check-binaries help

# Default target
all: build

# Build all components in release mode
build: build-release

# Build all components in debug mode (faster compilation)
build-debug: frontend service-debug client-debug cli-debug picker

# Build all components in release mode
build-release: frontend service-release client-release cli-release picker

# Build all components with CUDA acceleration (requires NVIDIA CUDA Toolkit on Linux)
build-cuda: frontend service-cuda client-release cli-release picker

# =============================================================================
# Individual Components
# =============================================================================

# Build frontend (TypeScript/Vite)
frontend:
	@echo "==> Building frontend..."
	pnpm build

# Build omnirec-service (debug)
service-debug:
	@echo "==> Building omnirec-service (debug)..."
	cd src-service && cargo build

# Build omnirec-service (release)
service-release:
	@echo "==> Building omnirec-service (release)..."
	cd src-service && cargo build --release

# Build omnirec-service with CUDA acceleration (release)
# Requires: NVIDIA CUDA Toolkit (nvcc, cuBLAS) on Linux
#           On Windows, prebuilt CUDA binaries are downloaded automatically
#           On macOS, this has no effect (Metal acceleration is always used)
service-cuda:
	@echo "==> Building omnirec-service with CUDA (release)..."
	cd src-service && cargo build --release --features cuda

# Alias for release
service: service-release

# Build omnirec client/Tauri app (debug)
client-debug:
	@echo "==> Building omnirec client (debug)..."
	cd src-tauri && cargo build

# Build omnirec client/Tauri app (release)
client-release:
	@echo "==> Building omnirec client (release)..."
	cd src-tauri && cargo build --release

# Alias for release
client: client-release

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
lint: lint-rust lint-ts

# Rust linting (all crates)
lint-rust:
	@echo "==> Linting src-common..."
	cd src-common && cargo clippy --all-targets --all-features -- -D warnings
	@echo "==> Linting src-service..."
	cd src-service && cargo clippy --all-targets --all-features -- -D warnings
	@echo "==> Linting src-tauri..."
	cd src-tauri && cargo clippy --all-targets --all-features -- -D warnings
	@echo "==> Linting src-cli..."
	cd src-cli && cargo clippy --all-targets --all-features -- -D warnings

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
	@echo "==> Testing src-service..."
	cd src-service && cargo test --all-features
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
	cd src-service && cargo clean
	cd src-tauri && cargo clean
	cd src-cli && cargo clean
	@echo "==> Cleaning picker..."
	rm -rf src-picker/build

# Clean only Rust debug builds (keeps release)
clean-debug:
	cd src-common && cargo clean --profile dev
	cd src-service && cargo clean --profile dev
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

# Build and run service in foreground (debug)
run-service: service-debug
	@echo "==> Running omnirec-service (debug)..."
	./target/debug/omnirec-service

# Build and run service in foreground (release)
run-service-release: service-release
	@echo "==> Running omnirec-service (release)..."
	./target/release/omnirec-service

# Build and run service with CUDA in foreground (release)
run-service-cuda: service-cuda
	@echo "==> Running omnirec-service with CUDA (release)..."
	./target/release/omnirec-service

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
	@test -f src-tauri/target/release/omnirec-app && echo "  [OK] omnirec-app" || echo "  [MISSING] omnirec-app"
	@test -f src-cli/target/release/omnirec && echo "  [OK] omnirec" || echo "  [MISSING] omnirec"
	@test -f src-service/target/release/omnirec-service && echo "  [OK] omnirec-service" || echo "  [MISSING] omnirec-service"
	@test -f src-picker/build/omnirec-picker && echo "  [OK] omnirec-picker" || echo "  [MISSING] omnirec-picker"

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
	@echo "  build-cuda       Build with CUDA GPU acceleration for transcription"
	@echo "  frontend         Build frontend only"
	@echo "  service          Build omnirec-service (release)"
	@echo "  service-debug    Build omnirec-service (debug)"
	@echo "  service-cuda     Build omnirec-service with CUDA (release)"
	@echo "  client           Build omnirec Tauri app (release)"
	@echo "  client-debug     Build omnirec Tauri app (debug)"
	@echo "  cli              Build omnirec CLI (release)"
	@echo "  cli-debug        Build omnirec CLI (debug)"
	@echo "  picker           Build omnirec-picker (C++/Qt6)"
	@echo ""
	@echo "Quality Targets:"
	@echo "  lint             Run all linters"
	@echo "  lint-rust        Run Rust clippy on all crates"
	@echo "  lint-ts          Run TypeScript type check"
	@echo "  test             Run all tests"
	@echo "  test-rust        Run Rust tests on all crates"
	@echo ""
	@echo "Run Targets:"
	@echo "  run-service         Build and run service (debug)"
	@echo "  run-service-release Build and run service (release)"
	@echo "  run-service-cuda    Build and run service with CUDA (release)"
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
	@echo "  The 'cuda' targets enable GPU-accelerated transcription via whisper.cpp."
	@echo "  - Linux: Requires NVIDIA CUDA Toolkit (nvcc, cuBLAS) at build time"
	@echo "  - Windows: Uses prebuilt CUDA binaries (downloaded automatically)"
	@echo "  - macOS: No effect (Metal acceleration is always used)"
