# Tasks: Add Windows IPC Support

## 1. Service Named Pipe Server

- [x] 1.1 Add tokio `windows-sys` feature for async named pipe support to `src-service/Cargo.toml`
- [x] 1.2 Create secure named pipe with SDDL security descriptor in `run_server()` Windows impl
- [x] 1.3 Implement async accept loop for named pipe connections
- [x] 1.4 Call `verify_peer()` with pipe handle before processing requests
- [x] 1.5 Reuse existing `handle_client()` logic (works with any AsyncRead+AsyncWrite)

## 2. Client Named Pipe Connection

- [x] 2.1 Implement `ConnectionState::Connected` variant for Windows in `src-tauri/src/ipc/client.rs`
- [x] 2.2 Implement `connect()` to open named pipe using std or tokio-based API
- [x] 2.3 Implement `request()` to send/receive over named pipe (mirrors Unix socket logic)
- [x] 2.4 Implement `is_service_available()` for Windows (check if pipe exists)
- [x] 2.5 Implement `wait_for_service()` for Windows (poll until pipe available)

## 3. Peer Verification (Windows)

- [x] 3.1 Verify `verify_peer()` in `src-common/src/security/peer_verify.rs` compiles and works
- [x] 3.2 Test GetNamedPipeClientProcessId returns correct PID
- [x] 3.3 Test QueryFullProcessImageNameW returns correct path
- [x] 3.4 Test executable verification accepts trusted binaries
- [x] 3.5 Test executable verification rejects untrusted binaries

## 4. Service Binary Discovery (Windows)

- [x] 4.1 Update `find_service_binary()` to check Windows-specific paths
- [x] 4.2 Add `.exe` suffix handling for Windows
- [x] 4.3 Test discovery from development target directory
- [x] 4.4 Test discovery from Program Files installation

## 5. Windows Dependencies

- [x] 5.1 Add `Win32_System_Pipes` feature to windows crate (already present)
- [x] 5.2 Add `Win32_Security` feature to windows crate (already present)
- [x] 5.3 Verify all required Windows API features are enabled

## 6. Integration Testing

- [x] 6.1 Test service starts and creates named pipe
- [x] 6.2 Test client connects to service via named pipe
- [x] 6.3 Test full request/response cycle (e.g., list_monitors)
- [x] 6.4 Test peer verification rejects connections from other processes
- [x] 6.5 Test graceful shutdown closes pipe and connections
- [x] 6.6 Test client can spawn service if not running
- [x] 6.7 Test recording workflow end-to-end on Windows

## 7. Documentation

- [x] 7.1 Update README with Windows-specific setup notes if any
- [x] 7.2 Document any Windows-specific limitations or known issues

## Dependencies

- Task 1 and Task 2 can be developed in parallel
- Task 3 is already partially implemented, needs verification
- Task 4 depends on Task 2
- Task 5 should be verified first (blocking)
- Task 6 requires Tasks 1-4 complete
- Task 7 after Task 6

## Validation

Run the following to verify changes:
```bash
# Build on Windows
cargo build --release -p omnirec-service
cargo build --release -p omnirec

# Run clippy
cargo clippy --all-targets -- -D warnings

# Run tests
cargo test --all-features

# Manual testing
./target/release/omnirec-service &
./target/release/omnirec
```
