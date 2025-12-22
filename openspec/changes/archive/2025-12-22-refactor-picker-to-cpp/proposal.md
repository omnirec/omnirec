# Change: Rewrite Portal Picker in C++

## Why

The current picker implementation is split between Rust (main logic, IPC client) and C++ (Qt dialog). This creates build complexity and duplicates dependencies. Consolidating to pure C++ simplifies the build process, reduces binary size by eliminating the Rust runtime, and allows reuse of the existing Qt dialog code directly within the picker.

## What Changes

- Rewrite `src-picker/` from Rust to C++ using Qt6
- Consolidate the separate `omnirec-dialog` into the main picker executable
- Implement IPC client using Qt's native Unix socket support
- Implement XDPH window list parsing and selection output formatting
- Maintain the same executable name (`omnirec-picker`)
- Remove Rust-based picker from cargo workspace
- Update CI workflows to build C++ picker instead of Rust
- Update release workflow to build and package C++ picker

## Impact

- Affected specs: wayland-portal (implementation change only, no behavior changes)
- Affected code:
  - `src-picker/` - Complete rewrite from Rust to C++
  - `.github/workflows/ci.yml` - Replace cargo commands with CMake
  - `.github/workflows/release.yml` - Replace cargo build with CMake
  - `packaging/aur/PKGBUILD` - Update build dependencies
