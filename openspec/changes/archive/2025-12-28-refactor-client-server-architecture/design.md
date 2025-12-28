# Design: Client-Server Architecture

## Context

OmniRec currently runs as a single Tauri application that handles both UI and capture/encoding. This design document outlines the separation into two components:

1. **omnirec** (Client) - Tauri application providing all UI functionality
2. **omnirec-service** (Service) - Background process handling capture, encoding, and state

### Stakeholders

- End users: No visible change in workflow
- Developers: New architecture to understand and maintain
- Packagers: Additional binary to include in distributions

## Goals / Non-Goals

### Goals

- Separate capture/encoding from UI into a dedicated service process
- Maintain identical user experience (no visible changes)
- Enable service to persist between UI sessions (optional, not required for MVP)
- Define a clean IPC protocol for client-service communication
- Preserve cross-platform support (Windows, Linux, macOS)
- **Secure the IPC channel against unauthorized local access**

### Non-Goals

- Multi-client support (multiple UIs connected simultaneously) - future work
- CLI interface for the service - future work
- Remote/network access to service - out of scope
- Persistent service (daemon/system service) - future work
- Changing the recording pipeline or codecs

## Security Model

### Threat Model

The IPC interface exposes sensitive capabilities that must be protected:

| Asset | Risk | Impact |
|-------|------|--------|
| Screen content | Unauthorized capture | Privacy violation |
| Audio content | Unauthorized recording | Privacy violation |
| Recording files | Unauthorized access/exfiltration | Data theft |
| Configuration | Unauthorized modification | Settings tampering |

**Primary Threat**: A malicious local process running as the same user could connect to the IPC socket and start unauthorized recordings, capture screen thumbnails, or access window enumeration data.

**Trust Boundary**: The IPC channel between client and service is the critical trust boundary. Every connection must be authenticated before any operations are permitted.

### Security Principles

1. **Authentication First**: Verify peer credentials before processing any IPC request
2. **Defense in Depth**: Multiple layers of protection (socket permissions + peer verification + input validation)
3. **Least Privilege**: Only return data the client needs; validate all inputs
4. **Fail Secure**: Reject connections by default; allow only verified peers

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    omnirec (Tauri Client)                    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                   Frontend (TS/HTML/CSS)             │    │
│  │  - Window/Display/Region selection UI                │    │
│  │  - Recording controls and status                     │    │
│  │  - Settings/Configuration UI                         │    │
│  │  - About dialog                                      │    │
│  └──────────────────────┬──────────────────────────────┘    │
│                         │ Tauri Commands                     │
│  ┌──────────────────────▼──────────────────────────────┐    │
│  │                   Rust Backend                       │    │
│  │  - IPC Client (authenticated connection to service)  │    │
│  │  - Tauri command handlers (proxy to service)         │    │
│  │  - System tray management                            │    │
│  │  - Region selector window                            │    │
│  │  - Service lifecycle (start on launch)               │    │
│  └──────────────────────┬──────────────────────────────┘    │
└─────────────────────────┼───────────────────────────────────┘
                          │ Authenticated IPC
                          │ (Unix socket / Named pipe)
┌─────────────────────────▼───────────────────────────────────┐
│                 omnirec-service (Background)                 │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              Secure IPC Server                       │    │
│  │  - Peer credential verification (MUST pass)          │    │
│  │  - Input validation layer                            │    │
│  │  - Request/Response handling                         │    │
│  │  - Event streaming                                   │    │
│  └──────────────────────┬──────────────────────────────┘    │
│                         │                                    │
│  ┌──────────────────────▼──────────────────────────────┐    │
│  │               Core Services                          │    │
│  │  ┌────────────┐  ┌────────────┐  ┌───────────────┐  │    │
│  │  │  Capture   │  │  Encoder   │  │ Audio Capture │  │    │
│  │  │  Backends  │  │  (FFmpeg)  │  │    & Mixing   │  │    │
│  │  └────────────┘  └────────────┘  └───────────────┘  │    │
│  │  ┌────────────────────────────────────────────────┐ │    │
│  │  │          Recording State Manager               │ │    │
│  │  └────────────────────────────────────────────────┘ │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## IPC Transport Layer

### Socket Configuration

The IPC channel uses local-only transports with restrictive permissions:

| Platform | Transport | Path | Permissions |
|----------|-----------|------|-------------|
| Linux | Unix domain socket | `$XDG_RUNTIME_DIR/omnirec/service.sock` | Socket: 0600, Dir: 0700 |
| macOS | Unix domain socket | `$TMPDIR/omnirec/service.sock` | Socket: 0600, Dir: 0700 |
| Windows | Named pipe | `\\.\pipe\omnirec-service` | Current user only, no network |

### Socket Setup (Service)

```rust
// src-service/src/ipc/transport.rs

use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const SOCKET_MODE: u32 = 0o600;   // Owner read/write only
const DIRECTORY_MODE: u32 = 0o700; // Owner read/write/execute only

/// Get the platform-specific socket path
pub fn get_socket_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
        PathBuf::from(runtime_dir).join("omnirec").join("service.sock")
    }
    
    #[cfg(target_os = "macos")]
    {
        let tmpdir = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(tmpdir).join("omnirec").join("service.sock")
    }
    
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(r"\\.\pipe\omnirec-service")
    }
}

/// Create socket directory with secure permissions
#[cfg(unix)]
pub fn create_secure_socket_dir(socket_path: &Path) -> std::io::Result<()> {
    let socket_dir = socket_path.parent().expect("Socket must have parent directory");
    
    // Create directory
    std::fs::create_dir_all(socket_dir)?;
    
    // Set restrictive permissions (0700)
    std::fs::set_permissions(socket_dir, std::fs::Permissions::from_mode(DIRECTORY_MODE))?;
    
    // Remove stale socket if exists
    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }
    
    Ok(())
}

/// Set socket file permissions after binding
#[cfg(unix)]
pub fn secure_socket_file(socket_path: &Path) -> std::io::Result<()> {
    std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(SOCKET_MODE))?;
    
    // Verify permissions were set
    let actual = std::fs::metadata(socket_path)?.permissions().mode() & 0o777;
    if actual != SOCKET_MODE {
        eprintln!("[Security] WARNING: Socket mode is {:o}, expected {:o}", actual, SOCKET_MODE);
    }
    
    Ok(())
}
```

### Windows Named Pipe Security

```rust
// src-service/src/ipc/transport_windows.rs

use windows::Win32::System::Pipes::*;
use windows::Win32::Security::*;

/// Create named pipe with security restricting access to current user only
pub fn create_secure_pipe() -> windows::core::Result<HANDLE> {
    // PIPE_REJECT_REMOTE_CLIENTS prevents network access
    let pipe = unsafe {
        CreateNamedPipeW(
            w!("\\\\.\\pipe\\omnirec-service"),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            65536,
            65536,
            0,
            None, // Default security (current user)
        )?
    };
    
    Ok(pipe)
}
```

## Connection Authentication

Every IPC connection MUST be authenticated by verifying the connecting process is a trusted OmniRec binary. **Unauthenticated connections are rejected immediately without processing any requests.**

### Trusted Binaries

```rust
// src-common/src/security/mod.rs

/// Executable names permitted to connect
pub const TRUSTED_EXECUTABLES: &[&str] = &[
    "omnirec",
    "omnirec-service",
    "omnirec-picker",
];

/// Trusted installation directories (platform-specific)
#[cfg(target_os = "linux")]
pub const TRUSTED_DIRECTORIES: &[&str] = &[
    "/usr/bin",
    "/usr/local/bin",
    "/opt/omnirec/bin",
];

#[cfg(target_os = "macos")]
pub const TRUSTED_DIRECTORIES: &[&str] = &[
    "/Applications/OmniRec.app/Contents/MacOS",
    "/usr/local/bin",
    "/opt/homebrew/bin",
];

#[cfg(target_os = "windows")]
pub const TRUSTED_DIRECTORIES: &[&str] = &[
    r"C:\Program Files\OmniRec",
    r"C:\Program Files (x86)\OmniRec",
];
```

### Peer Verification

```rust
// src-common/src/security/peer_verify.rs

use std::path::PathBuf;

#[derive(Debug)]
pub struct PeerInfo {
    pub pid: i32,
    pub executable: PathBuf,
}

#[derive(Debug)]
pub enum PeerVerifyError {
    CredentialsFailed(String),
    ProcessNotFound(i32),
    UntrustedExecutable(PathBuf),
    UntrustedDirectory(PathBuf),
    UidMismatch { peer: u32, current: u32 },
}

/// Verify connecting peer is a trusted OmniRec process
/// 
/// # Security
/// This function MUST be called on every new connection BEFORE
/// processing any IPC requests. Failure to verify results in
/// immediate connection termination.
#[cfg(target_os = "linux")]
pub fn verify_peer(stream: &std::os::unix::net::UnixStream) -> Result<PeerInfo, PeerVerifyError> {
    use std::os::unix::io::AsRawFd;
    
    // Get peer credentials via SO_PEERCRED
    let fd = stream.as_raw_fd();
    let creds = unsafe {
        let mut creds: libc::ucred = std::mem::zeroed();
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        
        if libc::getsockopt(fd, libc::SOL_SOCKET, libc::SO_PEERCRED,
            &mut creds as *mut _ as *mut _, &mut len) != 0 {
            return Err(PeerVerifyError::CredentialsFailed(
                std::io::Error::last_os_error().to_string()
            ));
        }
        creds
    };
    
    // Verify UID matches current user
    let current_uid = unsafe { libc::getuid() };
    if creds.uid != current_uid {
        return Err(PeerVerifyError::UidMismatch { 
            peer: creds.uid, 
            current: current_uid 
        });
    }
    
    // Get executable path from /proc
    let exe_path = std::fs::read_link(format!("/proc/{}/exe", creds.pid))
        .map_err(|_| PeerVerifyError::ProcessNotFound(creds.pid))?;
    
    // Verify executable name
    let exe_name = exe_path.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| PeerVerifyError::UntrustedExecutable(exe_path.clone()))?;
    
    if !TRUSTED_EXECUTABLES.contains(&exe_name) {
        return Err(PeerVerifyError::UntrustedExecutable(exe_path));
    }
    
    // Verify directory (trusted location OR same as service)
    let exe_dir = exe_path.parent()
        .ok_or_else(|| PeerVerifyError::UntrustedDirectory(exe_path.clone()))?;
    
    let self_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let in_trusted_dir = TRUSTED_DIRECTORIES.iter().any(|d| exe_dir == Path::new(d));
    let same_as_self = self_dir.as_ref().map(|d| exe_dir == d).unwrap_or(false);
    
    if !in_trusted_dir && !same_as_self {
        return Err(PeerVerifyError::UntrustedDirectory(exe_path));
    }
    
    Ok(PeerInfo { pid: creds.pid, executable: exe_path })
}

#[cfg(target_os = "macos")]
pub fn verify_peer(stream: &std::os::unix::net::UnixStream) -> Result<PeerInfo, PeerVerifyError> {
    use std::os::unix::io::AsRawFd;
    
    let fd = stream.as_raw_fd();
    
    // Get PID via LOCAL_PEERPID
    let pid = unsafe {
        let mut pid: libc::pid_t = 0;
        let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
        
        if libc::getsockopt(fd, libc::SOL_LOCAL, libc::LOCAL_PEERPID,
            &mut pid as *mut _ as *mut _, &mut len) != 0 {
            return Err(PeerVerifyError::CredentialsFailed(
                std::io::Error::last_os_error().to_string()
            ));
        }
        pid
    };
    
    // Get executable path via proc_pidpath
    let exe_path = {
        let mut buf = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        let len = unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr() as *mut _, buf.len() as u32) };
        if len <= 0 {
            return Err(PeerVerifyError::ProcessNotFound(pid));
        }
        PathBuf::from(std::str::from_utf8(&buf[..len as usize])
            .map_err(|_| PeerVerifyError::ProcessNotFound(pid))?)
    };
    
    // Same verification as Linux...
    verify_executable(&exe_path)?;
    
    Ok(PeerInfo { pid, executable: exe_path })
}

#[cfg(target_os = "windows")]
pub fn verify_peer(pipe: windows::Win32::Foundation::HANDLE) -> Result<PeerInfo, PeerVerifyError> {
    use windows::Win32::System::Pipes::GetNamedPipeClientProcessId;
    use windows::Win32::System::Threading::*;
    
    // Get client PID
    let mut pid: u32 = 0;
    unsafe {
        GetNamedPipeClientProcessId(pipe, &mut pid)
            .map_err(|e| PeerVerifyError::CredentialsFailed(e.to_string()))?;
    }
    
    // Get executable path
    let exe_path = {
        let process = unsafe {
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
                .map_err(|_| PeerVerifyError::ProcessNotFound(pid as i32))?
        };
        
        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        unsafe {
            QueryFullProcessImageNameW(process, Default::default(),
                windows::core::PWSTR(buf.as_mut_ptr()), &mut len)
                .map_err(|_| PeerVerifyError::ProcessNotFound(pid as i32))?;
            CloseHandle(process);
        }
        
        PathBuf::from(String::from_utf16_lossy(&buf[..len as usize]))
    };
    
    // Verify (case-insensitive on Windows)
    let exe_name = exe_path.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| PeerVerifyError::UntrustedExecutable(exe_path.clone()))?;
    
    if !TRUSTED_EXECUTABLES.iter().any(|t| {
        exe_name.eq_ignore_ascii_case(t) || exe_name.eq_ignore_ascii_case(&format!("{}.exe", t))
    }) {
        return Err(PeerVerifyError::UntrustedExecutable(exe_path));
    }
    
    Ok(PeerInfo { pid: pid as i32, executable: exe_path })
}
```

### IPC Server Accept Loop

```rust
// src-service/src/ipc/server.rs

use tokio::net::UnixListener;

pub async fn run_ipc_server() -> Result<(), Error> {
    let socket_path = get_socket_path();
    
    // Create secure socket directory
    create_secure_socket_dir(&socket_path)?;
    
    // Bind socket
    let listener = UnixListener::bind(&socket_path)?;
    
    // Set socket permissions AFTER binding
    secure_socket_file(&socket_path)?;
    
    eprintln!("[IPC] Server listening on {:?}", socket_path);
    
    loop {
        let (stream, _) = listener.accept().await?;
        
        // CRITICAL: Verify peer BEFORE any request processing
        let std_stream = stream.into_std()?;
        match verify_peer(&std_stream) {
            Ok(peer) => {
                eprintln!("[IPC] Authenticated: pid={} exe={:?}", peer.pid, peer.executable);
                let stream = tokio::net::UnixStream::from_std(std_stream)?;
                tokio::spawn(handle_authenticated_client(stream, peer));
            }
            Err(e) => {
                eprintln!("[IPC] REJECTED: {:?}", e);
                // Connection dropped - stream goes out of scope
            }
        }
    }
}
```

## Input Validation Layer

All IPC request parameters are validated before processing. Invalid input results in an error response without executing the requested operation.

### Identifier Validation

Identifiers (monitor_id, source_id) must match strict patterns to prevent path injection:

```rust
// src-common/src/security/validation.rs

use regex::Regex;
use once_cell::sync::Lazy;

/// Monitor ID: alphanumeric, dash, underscore (e.g., "DP-1", "HDMI-A-1")
static MONITOR_ID_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[A-Za-z0-9_-]{1,64}$").unwrap()
});

/// Audio source ID: allows dots and colons for PipeWire IDs
static SOURCE_ID_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[A-Za-z0-9_.\-:]{1,256}$").unwrap()
});

#[derive(Debug)]
pub enum ValidationError {
    InvalidMonitorId(String),
    InvalidSourceId(String),
    InvalidWindowHandle(isize),
    DimensionOutOfRange { field: &'static str, value: u32, max: u32 },
    CoordinateOutOfRange { field: &'static str, value: i32 },
    StringTooLong { field: &'static str, len: usize, max: usize },
    MessageTooLarge { size: usize, max: usize },
}

pub fn validate_monitor_id(id: &str) -> Result<(), ValidationError> {
    if !MONITOR_ID_PATTERN.is_match(id) {
        return Err(ValidationError::InvalidMonitorId(id.to_string()));
    }
    Ok(())
}

pub fn validate_source_id(id: &str) -> Result<(), ValidationError> {
    if !SOURCE_ID_PATTERN.is_match(id) {
        return Err(ValidationError::InvalidSourceId(id.to_string()));
    }
    Ok(())
}

pub fn validate_window_handle(handle: isize) -> Result<(), ValidationError> {
    if handle < 0 {
        return Err(ValidationError::InvalidWindowHandle(handle));
    }
    Ok(())
}

pub fn validate_dimensions(width: u32, height: u32) -> Result<(), ValidationError> {
    const MAX_DIMENSION: u32 = 16384;
    
    if width == 0 || width > MAX_DIMENSION {
        return Err(ValidationError::DimensionOutOfRange { 
            field: "width", value: width, max: MAX_DIMENSION 
        });
    }
    if height == 0 || height > MAX_DIMENSION {
        return Err(ValidationError::DimensionOutOfRange { 
            field: "height", value: height, max: MAX_DIMENSION 
        });
    }
    Ok(())
}

pub fn validate_coordinates(x: i32, y: i32) -> Result<(), ValidationError> {
    const MAX_COORD: i32 = 65535;
    const MIN_COORD: i32 = -65535;
    
    if x < MIN_COORD || x > MAX_COORD {
        return Err(ValidationError::CoordinateOutOfRange { field: "x", value: x });
    }
    if y < MIN_COORD || y > MAX_COORD {
        return Err(ValidationError::CoordinateOutOfRange { field: "y", value: y });
    }
    Ok(())
}
```

### Path Validation

Output directory paths are canonicalized and verified against allowed locations:

```rust
// src-common/src/security/path_validation.rs

use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum PathError {
    ContainsTraversal,
    ContainsNullByte,
    NotAbsolute,
    OutsideAllowedDirectory,
    TooLong(usize),
}

/// Validate and canonicalize an output directory path
pub fn validate_output_directory(path: &Path) -> Result<PathBuf, PathError> {
    let path_str = path.to_string_lossy();
    
    // Check for null bytes
    if path_str.contains('\0') {
        return Err(PathError::ContainsNullByte);
    }
    
    // Check length
    if path_str.len() > 4096 {
        return Err(PathError::TooLong(path_str.len()));
    }
    
    // Reject traversal sequences before canonicalization
    for component in path.components() {
        if component.as_os_str() == ".." {
            return Err(PathError::ContainsTraversal);
        }
    }
    
    // Must be absolute
    if !path.is_absolute() {
        return Err(PathError::NotAbsolute);
    }
    
    // Canonicalize to resolve symlinks
    let canonical = std::fs::canonicalize(path)
        .map_err(|_| PathError::OutsideAllowedDirectory)?;
    
    // Verify within allowed directories
    let allowed = get_allowed_output_dirs();
    let in_allowed = allowed.iter().any(|base| {
        std::fs::canonicalize(base)
            .map(|b| canonical.starts_with(&b))
            .unwrap_or(false)
    });
    
    if !in_allowed {
        return Err(PathError::OutsideAllowedDirectory);
    }
    
    Ok(canonical)
}

fn get_allowed_output_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.clone());
        dirs.push(home.join("Videos"));
        dirs.push(home.join("Movies"));
        dirs.push(home.join("Documents"));
        dirs.push(home.join("Desktop"));
    }
    
    if let Some(video) = dirs::video_dir() {
        dirs.push(video);
    }
    
    dirs.push(PathBuf::from("/tmp"));
    
    dirs
}
```

### Message Size Limits

```rust
// src-common/src/ipc/protocol.rs

/// Maximum IPC message size (64 KB)
pub const MAX_MESSAGE_SIZE: usize = 65536;

/// Read a length-prefixed message with size validation
pub async fn read_message<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, IpcError> {
    // Read 4-byte length prefix
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    
    // Validate size BEFORE allocating
    if len > MAX_MESSAGE_SIZE {
        return Err(IpcError::MessageTooLarge { size: len, max: MAX_MESSAGE_SIZE });
    }
    
    // Read payload
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    
    Ok(buf)
}
```

## IPC Protocol

### Message Format

```
┌──────────────────┬─────────────────────────────────┐
│ Length (4 bytes) │ JSON Payload (variable length)  │
│ Little-endian    │ Max 65,536 bytes                │
└──────────────────┴─────────────────────────────────┘
```

### Request Types

```rust
// src-common/src/ipc/requests.rs

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    // Enumeration
    ListWindows,
    ListMonitors,
    ListAudioSources,
    
    // Capture control
    StartWindowCapture { window_handle: isize },
    StartDisplayCapture { monitor_id: String, width: u32, height: u32 },
    StartRegionCapture { monitor_id: String, x: i32, y: i32, width: u32, height: u32 },
    StartPortalCapture,
    StopRecording,
    
    // State
    GetRecordingState,
    GetElapsedTime,
    SubscribeEvents,
    
    // Configuration
    GetOutputFormat,
    SetOutputFormat { format: String },
    GetAudioConfig,
    SetAudioConfig { enabled: bool, source_id: Option<String>, microphone_id: Option<String> },
    
    // Thumbnails
    GetWindowThumbnail { window_handle: isize },
    GetDisplayThumbnail { monitor_id: String },
    GetRegionPreview { monitor_id: String, x: i32, y: i32, width: u32, height: u32 },
    
    // Highlights
    ShowDisplayHighlight { x: i32, y: i32, width: i32, height: i32 },
    ShowWindowHighlight { window_handle: isize },
    
    // Picker compatibility
    QuerySelection,
    ValidateToken { token: String },
    StoreToken { token: String },
}
```

### Request Validation

Each request type has a validation function:

```rust
// src-service/src/ipc/handlers.rs

impl Request {
    /// Validate all parameters in this request
    pub fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Request::StartWindowCapture { window_handle } => {
                validate_window_handle(*window_handle)?;
            }
            Request::StartDisplayCapture { monitor_id, width, height } => {
                validate_monitor_id(monitor_id)?;
                validate_dimensions(*width, *height)?;
            }
            Request::StartRegionCapture { monitor_id, x, y, width, height } => {
                validate_monitor_id(monitor_id)?;
                validate_coordinates(*x, *y)?;
                validate_dimensions(*width, *height)?;
            }
            Request::GetWindowThumbnail { window_handle } => {
                validate_window_handle(*window_handle)?;
            }
            Request::GetDisplayThumbnail { monitor_id } => {
                validate_monitor_id(monitor_id)?;
            }
            Request::GetRegionPreview { monitor_id, x, y, width, height } => {
                validate_monitor_id(monitor_id)?;
                validate_coordinates(*x, *y)?;
                validate_dimensions(*width, *height)?;
            }
            Request::ShowWindowHighlight { window_handle } => {
                validate_window_handle(*window_handle)?;
            }
            Request::SetAudioConfig { source_id, microphone_id, .. } => {
                if let Some(id) = source_id {
                    validate_source_id(id)?;
                }
                if let Some(id) = microphone_id {
                    validate_source_id(id)?;
                }
            }
            // Other requests have no parameters to validate
            _ => {}
        }
        Ok(())
    }
}
```

### Response Types

```rust
// src-common/src/ipc/responses.rs

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    // Success responses
    Windows { windows: Vec<WindowInfo> },
    Monitors { monitors: Vec<MonitorInfo> },
    AudioSources { sources: Vec<AudioSource> },
    RecordingState { state: String },
    RecordingStarted,
    RecordingStopped { file_path: String, source_path: String },
    ElapsedTime { seconds: u64 },
    OutputFormat { format: String },
    AudioConfig { enabled: bool, source_id: Option<String>, microphone_id: Option<String> },
    Thumbnail { data: String, width: u32, height: u32 },
    Subscribed,
    Ok,
    
    // Selection (picker)
    Selection { source_type: String, source_id: String, has_approval_token: bool },
    NoSelection,
    TokenValid,
    TokenInvalid,
    TokenStored,
    
    // Error (sanitized - no internal details)
    Error { message: String },
    
    // Events (after subscribe)
    Event { event: String, data: serde_json::Value },
}
```

### Error Sanitization

Error messages returned to clients are sanitized to prevent information disclosure:

```rust
// src-service/src/ipc/errors.rs

/// Convert internal errors to client-safe messages
pub fn sanitize_error(err: &ServiceError) -> String {
    match err {
        ServiceError::NotRecording => "Not currently recording".to_string(),
        ServiceError::AlreadyRecording => "Already recording".to_string(),
        ServiceError::InvalidFormat(f) => format!("Invalid format: {}", f),
        ServiceError::ValidationError(e) => format!("Invalid input: {:?}", e),
        ServiceError::CaptureError(_) => "Capture failed".to_string(),
        ServiceError::EncoderError(_) => "Encoding failed".to_string(),
        ServiceError::PermissionDenied => "Permission denied".to_string(),
        // Never expose file paths or internal details
        _ => "Internal error".to_string(),
    }
}
```

## Service Lifecycle

### Client Startup Flow

```
┌─────────────────┐
│  Client starts  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐     ┌──────────────────┐
│ Connect to      │────►│ Service running? │
│ service socket  │     └────────┬─────────┘
└─────────────────┘              │
         │                       │ No
         │ Yes                   ▼
         │              ┌──────────────────┐
         │              │ Spawn service    │
         │              │ process          │
         │              └────────┬─────────┘
         │                       │
         │                       ▼
         │              ┌──────────────────┐
         │              │ Wait for socket  │
         │              │ (with timeout)   │
         │              └────────┬─────────┘
         │                       │
         ▼                       ▼
┌─────────────────────────────────────────┐
│           Connected to service           │
│    (Peer verification happens here)      │
└─────────────────────────────────────────┘
```

### Service Startup

```rust
// src-service/src/main.rs

fn main() {
    // Initialize logging
    env_logger::init();
    
    // Initialize FFmpeg
    if let Err(e) = ensure_ffmpeg() {
        eprintln!("[Service] FFmpeg initialization failed: {}", e);
        std::process::exit(1);
    }
    
    // Initialize platform capture backends
    init_capture_backends();
    
    // Run async runtime
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            if let Err(e) = run_ipc_server().await {
                eprintln!("[Service] IPC server error: {}", e);
                std::process::exit(1);
            }
        });
}
```

## Project Structure

```
omnirec/
├── src-tauri/                  # Tauri client
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── commands/           # Tauri commands (proxy to service)
│       ├── ipc_client.rs       # IPC client implementation
│       └── tray/               # System tray
├── src-common/                 # Shared types and IPC protocol
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs            # WindowInfo, MonitorInfo, etc.
│       ├── ipc/
│       │   ├── protocol.rs
│       │   ├── requests.rs
│       │   └── responses.rs
│       └── security/
│           ├── mod.rs
│           ├── peer_verify.rs
│           ├── validation.rs
│           └── path_validation.rs
├── src-service/                # Background service
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── ipc/
│       │   ├── server.rs       # IPC server with auth
│       │   ├── handlers.rs     # Request handlers
│       │   └── mod.rs
│       ├── capture/            # Migrated from main app
│       ├── encoder/            # Migrated from main app
│       └── state.rs            # Recording state
└── src-picker/                 # Custom picker (C++/Qt6)
    ├── CMakeLists.txt
    └── *.cpp/h
```

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Malicious local process connects | HIGH | Peer credential verification rejects unauthorized connections |
| Path traversal in parameters | MEDIUM | Strict identifier validation, path canonicalization |
| Resource exhaustion (DoS) | MEDIUM | Message size limits, rate limiting (P1) |
| Service crash during recording | MEDIUM | Graceful error handling, partial file saved |
| Debugging difficulty | LOW | JSON protocol is human-readable, both processes log to stderr |

## Future Enhancements (Post-MVP)

1. **Rate limiting**: Per-client request rate limiting (100 req/sec)
2. **Connection limits**: Maximum 5 concurrent clients
3. **Read timeouts**: 30-second timeout on client reads
4. **Event backpressure**: Drop old events if client isn't reading
5. **Binary hash verification**: Verify service binary hash before spawning

## Open Questions

1. **Service persistence**: Should service stay running after last client disconnects?
   - Proposal: Exit after 60 seconds idle (allows quick reconnection)

2. **Multiple client support**: Future work - what isolation is needed between clients?
   - Proposal: Single active recording, shared state visible to all authenticated clients

3. **Picker integration**: Should picker connect to same service?
   - Proposal: Yes, same socket, same authentication
