# Change: Add Windows IPC Support

## Why

The client-server architecture was designed to be cross-platform, but the Windows named pipe implementation is currently stubbed out. The service immediately exits on Windows with an error message, and the client cannot connect. This blocks all OmniRec functionality on Windows despite having complete Windows capture backend implementations (window/display/region capture, audio, thumbnails, highlights).

## What Changes

- **MODIFIED** `service-architecture`: Implement Windows named pipe server with proper security
- **MODIFIED** `ipc-interface`: Ensure protocol works correctly over named pipes (message boundaries, security)
- Implement Windows named pipe server in `src-service/src/ipc/server.rs`
- Implement Windows named pipe client in `src-tauri/src/ipc/client.rs`
- Add Windows-specific peer credential verification
- Configure named pipe security to restrict access to current user

## Impact

- Affected specs: `service-architecture`, `ipc-interface`
- Affected code:
  - `src-service/src/ipc/server.rs` - Named pipe server implementation
  - `src-tauri/src/ipc/client.rs` - Named pipe client connection
  - `src-common/src/ipc/protocol.rs` - Already supports Windows path
  - `src-common/src/security/peer_verify.rs` - Already has Windows stub
- No changes to capture backends (already implemented for Windows)
- No changes to IPC protocol format (length-prefixed JSON works on any stream)
