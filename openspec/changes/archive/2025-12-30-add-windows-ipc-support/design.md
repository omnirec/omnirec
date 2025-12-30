# Design: Windows IPC Support

## Context

OmniRec uses a client-server architecture where:
- **Service** (`omnirec-service`): Background process handling capture, encoding, and recording
- **Client** (`omnirec` Tauri app): UI that communicates with the service via IPC

On Unix platforms (Linux/macOS), IPC uses Unix domain sockets. On Windows, the equivalent is named pipes. The architecture already accounts for this (socket path returns `\\.\pipe\omnirec-service`), but the server and client implementations are stubbed.

## Goals

- Enable full OmniRec functionality on Windows
- Maintain security parity with Unix implementation (current user only)
- Keep implementation simple and consistent with existing patterns

## Non-Goals

- Network access to the service (local only)
- Multiple concurrent clients (single-instance model)
- Changes to the IPC protocol (JSON over length-prefixed stream)

## Decisions

### Decision 1: Use `windows` crate async named pipes

**What**: Use `windows::Win32::System::Pipes` with Tokio async support via `tokio::net::windows::named_pipe`.

**Why**: 
- The `windows` crate is already a dependency for capture backends
- Tokio provides async named pipe support in `tokio::net::windows::named_pipe`
- Consistent with the async architecture used on Unix

**Alternatives considered**:
- `interprocess` crate: Adds another dependency; the windows crate already suffices
- Blocking I/O with thread pool: Inconsistent with existing async server design

### Decision 2: Named pipe security descriptor

**What**: Create the pipe with a security descriptor that restricts access to the current user (SDDL string or explicit ACL).

**Why**: Matches Unix socket behavior (file permissions restrict to owner) and prevents other users on shared systems from connecting.

**Implementation**:
```rust
use windows::Win32::Security::{
    InitializeSecurityDescriptor, SetSecurityDescriptorDacl,
    PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
};

// SDDL: D:(A;;GA;;;CU) = Allow Generic All to Current User
let sddl = "D:(A;;GA;;;CU)";
```

### Decision 3: Pipe naming convention

**What**: Use `\\.\pipe\omnirec-service` as the pipe name.

**Why**: 
- Already defined in `get_socket_path()`
- Simple, unique name that won't conflict with other applications
- No need for user-specific paths since security descriptor handles access control

### Decision 4: Peer verification approach

**What**: Use `GetNamedPipeClientProcessId` to get the client PID, then `QueryFullProcessImageNameW` to get the executable path for verification.

**Why**:
- Already partially implemented in `src-common/src/security/peer_verify.rs`
- Consistent with Unix approach (verify executable is a trusted OmniRec binary)
- Windows provides the necessary APIs without elevated privileges

### Decision 5: Handle message boundaries

**What**: The existing length-prefixed framing (4-byte LE length + JSON payload) works unchanged on named pipes.

**Why**: Named pipes in message mode maintain message boundaries, but byte mode with length prefixing is more portable and already implemented. The current protocol works on any byte stream.

## Architecture

```
                    Windows
                    ========
                    
    +--------------+                  +------------------+
    | omnirec.exe  |   Named Pipe     | omnirec-service  |
    | (Tauri UI)   | <--------------> | (Background)     |
    +--------------+  \\.\pipe\...    +------------------+
         |                                    |
         | Length-prefixed JSON               | Peer verification
         | (same as Unix)                     | (GetNamedPipeClientProcessId)
         |                                    |
         v                                    v
    +-------------------+            +-------------------+
    | ServiceClient     |            | Named Pipe Server |
    | (connect, request)|            | (listen, accept)  |
    +-------------------+            +-------------------+
```

## Security Model

| Aspect | Unix (socket) | Windows (named pipe) |
|--------|--------------|---------------------|
| Path | `$XDG_RUNTIME_DIR/omnirec/service.sock` | `\\.\pipe\omnirec-service` |
| Access control | File permissions (0600) | Security descriptor (current user) |
| Peer verification | SO_PEERCRED / LOCAL_PEERPID | GetNamedPipeClientProcessId |
| Executable check | /proc/{pid}/exe | QueryFullProcessImageNameW |

## Risks / Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| Named pipe race condition on creation | Another process could create pipe first | Verify pipe creator is our service before connecting |
| Security descriptor complexity | Potential for misconfiguration | Use well-known SDDL pattern |
| Tokio named pipe stability | API may change | Pin tokio version, use stable features |

## Open Questions

1. Should we add a retry mechanism if the pipe doesn't exist when client starts? (Current Unix behavior: wait up to 10s)
   - **Answer**: Yes, use the same `wait_for_service` pattern
