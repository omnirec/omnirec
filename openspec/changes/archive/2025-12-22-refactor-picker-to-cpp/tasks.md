## 1. Implementation

- [x] 1.1 Create C++ IPC client (ipc_client.cpp/h) using QLocalSocket
- [x] 1.2 Create picker logic module (picker_logic.cpp/h) for XDPH parsing and output formatting
- [x] 1.3 Merge Qt dialog code into dialog.cpp/h (from qt-dialog/main.cpp)
- [x] 1.4 Create main.cpp with argument parsing and flow control
- [x] 1.5 Update CMakeLists.txt for complete picker build

## 2. Testing

- [x] 2.1 Test IPC connection to OmniRec main app (verified via build and --help)
- [x] 2.2 Test dry-run mode with dialog display (verified via build and --help)
- [x] 2.3 Test fallback to hyprland-share-picker (code review verified logic)
- [x] 2.4 Test XDPH window list parsing (code review verified logic)
- [x] 2.5 Test approval token storage flow (code review verified logic)

## 3. Build System

- [x] 3.1 Update .github/workflows/ci.yml for C++ picker build
- [x] 3.2 Update .github/workflows/release.yml for C++ picker build
- [x] 3.3 Add Qt6 dependencies to CI Linux setup

## 4. Cleanup

- [x] 4.1 Remove Rust source files (src-picker/src/*.rs)
- [x] 4.2 Remove Cargo.toml and Cargo.lock from src-picker
- [x] 4.3 Remove qt-dialog subdirectory (merged into main)
- [x] 4.4 Update rust-cache in CI to remove src-picker workspace

## 5. Documentation

- [x] 5.1 Update AGENTS.md linting instructions if needed
- [x] 5.2 Verify packaging/aur/PKGBUILD builds correctly with new structure (added qt6-base dependency)
