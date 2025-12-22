## Context

The omnirec-picker is a Linux-only utility invoked by xdg-desktop-portal-hyprland (XDPH) when a screencast request needs source selection. It currently consists of:

1. **Rust picker** (`src-picker/src/`) - Main logic, IPC client, argument parsing
2. **C++ Qt dialog** (`src-picker/qt-dialog/`) - Permission approval UI

This split requires building both Rust and C++ toolchains on Linux, and the two binaries must be distributed together.

## Goals / Non-Goals

**Goals:**
- Single C++ codebase for the picker using Qt6
- Eliminate Rust dependency for the picker component
- Maintain identical behavior and command-line interface
- Reuse existing Qt dialog code directly
- Keep the same executable name (`omnirec-picker`)
- Simplify CI/CD by removing Rust steps for picker

**Non-Goals:**
- Add new features or change picker behavior
- Change the IPC protocol between picker and OmniRec
- Modify the Qt dialog appearance or UX
- Support platforms other than Linux

## Decisions

### Decision: Use Qt6 for all functionality

**Rationale:** Qt6 provides all required capabilities:
- `QLocalSocket` for Unix domain socket IPC
- JSON parsing via `QJsonDocument`
- Native subprocess execution via `QProcess`
- The existing dialog code already uses Qt6

**Alternatives considered:**
- Plain C++ with separate libraries (boost, nlohmann/json) - More dependencies, harder to integrate with Qt dialog
- Keep Rust, embed Qt dialog via FFI - Still requires two toolchains

### Decision: Single executable with embedded dialog

**Rationale:** The picker and dialog are always used together. Combining them eliminates the need to locate and invoke a separate dialog binary.

**Trade-off:** Slightly larger single binary vs. simpler deployment

### Decision: Keep same IPC protocol

**Rationale:** The OmniRec main app (Rust/Tauri) implements the IPC server. Keeping the same JSON-based protocol means no changes needed to the main app.

## File Structure

```
src-picker/
  CMakeLists.txt           # Main build file
  main.cpp                  # Entry point, argument parsing
  ipc_client.cpp/h          # Unix socket IPC client
  picker_logic.cpp/h        # XDPH parsing, output formatting
  dialog.cpp/h              # Permission dialog (merged from qt-dialog)
```

## Build Integration

### CMakeLists.txt structure
```cmake
cmake_minimum_required(VERSION 3.16)
project(omnirec-picker LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 REQUIRED COMPONENTS Widgets Network)

add_executable(omnirec-picker
    main.cpp
    ipc_client.cpp
    picker_logic.cpp
    dialog.cpp
)

target_link_libraries(omnirec-picker PRIVATE
    Qt6::Widgets
    Qt6::Network
)

install(TARGETS omnirec-picker RUNTIME DESTINATION bin)
```

### CI Changes
Replace Rust picker steps:
```yaml
# Before
- name: Rust lint (clippy) - picker
  working-directory: src-picker
  run: cargo clippy --all-targets --all-features -- -D warnings

# After
- name: Build picker
  run: |
    cmake -B src-picker/build -S src-picker
    cmake --build src-picker/build
```

## Risks / Trade-offs

- **Risk:** Qt version differences across distros
  - Mitigation: CI tests on Ubuntu 22.04 which uses Qt6
  - Mitigation: Specify Qt6 minimum version in CMakeLists.txt

- **Trade-off:** Losing Rust's memory safety guarantees
  - Mitigation: Simple, focused code with minimal dynamic memory
  - Mitigation: Code is short-lived (picker exits after selection)

## Migration Plan

1. Create new C++ source files in `src-picker/`
2. Update CMakeLists.txt to build picker instead of dialog
3. Verify functionality locally with `--dry-run` mode
4. Update CI workflows
5. Remove old Rust source files and Cargo.toml
6. Update release workflow for C++ build
7. Remove `qt-dialog/` subdirectory (merged into main)

## Open Questions

None - all design decisions are straightforward translations of existing behavior.
