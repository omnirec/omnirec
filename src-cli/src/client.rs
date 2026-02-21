//! IPC client for communicating with the OmniRec Tauri app.
//!
//! The CLI connects to the Tauri app via IPC socket. If the app is not running,
//! it spawns the app in headless mode (--headless) which runs tray-only.

use omnirec_common::ipc::{Request, Response, MAX_MESSAGE_SIZE};
use std::io::{Read, Write};
use std::time::Duration;
use tokio::sync::Mutex;

use crate::exit_codes::ExitCode;

/// Error type for service client operations.
#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum ServiceError {
    /// Service is not running or not connected
    NotConnected,
    /// Connection to service failed
    ConnectionFailed(String),
    /// Failed to send request
    SendFailed(String),
    /// Failed to receive response
    ReceiveFailed(String),
    /// Service returned an error
    RemoteError(String),
    /// Request timed out
    Timeout,
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceError::NotConnected => write!(f, "Not connected to service"),
            ServiceError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            ServiceError::SendFailed(msg) => write!(f, "Send failed: {}", msg),
            ServiceError::ReceiveFailed(msg) => write!(f, "Receive failed: {}", msg),
            ServiceError::RemoteError(msg) => write!(f, "Service error: {}", msg),
            ServiceError::Timeout => write!(f, "Request timed out"),
        }
    }
}

impl std::error::Error for ServiceError {}

impl ServiceError {
    /// Convert to an appropriate exit code.
    pub fn to_exit_code(&self) -> ExitCode {
        match self {
            ServiceError::NotConnected
            | ServiceError::ConnectionFailed(_)
            | ServiceError::Timeout => ExitCode::ServiceConnectionFailed,
            ServiceError::SendFailed(_) | ServiceError::ReceiveFailed(_) => {
                ExitCode::ServiceConnectionFailed
            }
            ServiceError::RemoteError(msg) => {
                // Try to determine a more specific exit code based on error message
                if msg.contains("not recording") || msg.contains("no recording") {
                    ExitCode::Success // Not an error for stop when not recording
                } else if msg.contains("invalid") || msg.contains("not found") {
                    ExitCode::RecordingFailedToStart
                } else {
                    ExitCode::GeneralError
                }
            }
        }
    }
}

/// Connection state for the service client.
enum ConnectionState {
    Disconnected,
    #[cfg(unix)]
    Connected(std::os::unix::net::UnixStream),
    #[cfg(windows)]
    Connected(std::fs::File),
}

/// Client for communicating with the OmniRec service.
pub struct ServiceClient {
    connection: Mutex<ConnectionState>,
    #[cfg(unix)]
    socket_path: std::path::PathBuf,
}

impl ServiceClient {
    /// Create a new service client.
    pub fn new() -> Self {
        Self {
            connection: Mutex::new(ConnectionState::Disconnected),
            #[cfg(unix)]
            socket_path: omnirec_common::ipc::get_socket_path(),
        }
    }

    /// Check if the client is connected to the service.
    pub async fn is_connected(&self) -> bool {
        let conn = self.connection.lock().await;
        !matches!(*conn, ConnectionState::Disconnected)
    }

    /// Connect to the service.
    pub async fn connect(&self) -> Result<(), ServiceError> {
        let mut conn = self.connection.lock().await;

        // Already connected?
        if !matches!(*conn, ConnectionState::Disconnected) {
            return Ok(());
        }

        #[cfg(unix)]
        {
            use std::os::unix::net::UnixStream;

            let stream = UnixStream::connect(&self.socket_path).map_err(|e| {
                ServiceError::ConnectionFailed(format!(
                    "Failed to connect to {}: {}",
                    self.socket_path.display(),
                    e
                ))
            })?;

            // Set read/write timeouts
            stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
            stream.set_write_timeout(Some(Duration::from_secs(10))).ok();

            *conn = ConnectionState::Connected(stream);
            Ok(())
        }

        #[cfg(windows)]
        {
            use std::fs::OpenOptions;

            let pipe_path = r"\\.\pipe\omnirec-service";

            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(pipe_path)
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        ServiceError::ConnectionFailed(
                            "Service not running (named pipe not found)".to_string(),
                        )
                    } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                        ServiceError::ConnectionFailed(
                            "Permission denied accessing named pipe".to_string(),
                        )
                    } else {
                        ServiceError::ConnectionFailed(format!(
                            "Failed to connect to {}: {}",
                            pipe_path, e
                        ))
                    }
                })?;

            *conn = ConnectionState::Connected(file);
            Ok(())
        }
    }

    /// Disconnect from the service.
    #[allow(dead_code)]
    pub async fn disconnect(&self) {
        let mut conn = self.connection.lock().await;
        *conn = ConnectionState::Disconnected;
    }

    /// Check if the service is available (Windows: named pipe exists).
    #[cfg(windows)]
    fn is_service_available(&self) -> bool {
        let pipe_path = r"\\.\pipe\omnirec-service";
        std::path::Path::new(pipe_path).exists()
    }

    /// Send a request to the service and wait for a response.
    pub async fn request(&self, request: Request) -> Result<Response, ServiceError> {
        // Ensure connected
        if !self.is_connected().await {
            self.connect().await?;
        }

        let mut conn = self.connection.lock().await;

        #[cfg(unix)]
        {
            let stream = match &mut *conn {
                ConnectionState::Connected(s) => s,
                ConnectionState::Disconnected => {
                    return Err(ServiceError::NotConnected);
                }
            };

            // Serialize request
            let request_json = serde_json::to_vec(&request).map_err(|e| {
                ServiceError::SendFailed(format!("Failed to serialize request: {}", e))
            })?;

            // Send length-prefixed message
            let len = request_json.len() as u32;
            stream
                .write_all(&len.to_le_bytes())
                .map_err(|e| ServiceError::SendFailed(format!("Failed to write length: {}", e)))?;
            stream
                .write_all(&request_json)
                .map_err(|e| ServiceError::SendFailed(format!("Failed to write request: {}", e)))?;
            stream
                .flush()
                .map_err(|e| ServiceError::SendFailed(format!("Failed to flush: {}", e)))?;

            // Read response length
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).map_err(|e| {
                ServiceError::ReceiveFailed(format!("Failed to read response length: {}", e))
            })?;
            let response_len = u32::from_le_bytes(len_buf) as usize;

            // Validate response length
            if response_len > MAX_MESSAGE_SIZE {
                return Err(ServiceError::ReceiveFailed(format!(
                    "Response too large: {} bytes",
                    response_len
                )));
            }

            // Read response body
            let mut response_buf = vec![0u8; response_len];
            stream.read_exact(&mut response_buf).map_err(|e| {
                ServiceError::ReceiveFailed(format!("Failed to read response: {}", e))
            })?;

            // Deserialize response
            let response: Response = serde_json::from_slice(&response_buf).map_err(|e| {
                ServiceError::ReceiveFailed(format!("Failed to deserialize response: {}", e))
            })?;

            // Check for service error
            if let Response::Error { message } = &response {
                return Err(ServiceError::RemoteError(message.clone()));
            }

            Ok(response)
        }

        #[cfg(windows)]
        {
            let file = match &mut *conn {
                ConnectionState::Connected(f) => f,
                ConnectionState::Disconnected => {
                    return Err(ServiceError::NotConnected);
                }
            };

            // Serialize request
            let request_json = serde_json::to_vec(&request).map_err(|e| {
                ServiceError::SendFailed(format!("Failed to serialize request: {}", e))
            })?;

            // Send length-prefixed message
            let len = request_json.len() as u32;
            file.write_all(&len.to_le_bytes())
                .map_err(|e| ServiceError::SendFailed(format!("Failed to write length: {}", e)))?;
            file.write_all(&request_json)
                .map_err(|e| ServiceError::SendFailed(format!("Failed to write request: {}", e)))?;
            file.flush()
                .map_err(|e| ServiceError::SendFailed(format!("Failed to flush: {}", e)))?;

            // Read response length
            let mut len_buf = [0u8; 4];
            file.read_exact(&mut len_buf).map_err(|e| {
                ServiceError::ReceiveFailed(format!("Failed to read response length: {}", e))
            })?;
            let response_len = u32::from_le_bytes(len_buf) as usize;

            // Validate response length
            if response_len > MAX_MESSAGE_SIZE {
                return Err(ServiceError::ReceiveFailed(format!(
                    "Response too large: {} bytes",
                    response_len
                )));
            }

            // Read response body
            let mut response_buf = vec![0u8; response_len];
            file.read_exact(&mut response_buf).map_err(|e| {
                ServiceError::ReceiveFailed(format!("Failed to read response: {}", e))
            })?;

            // Deserialize response
            let response: Response = serde_json::from_slice(&response_buf).map_err(|e| {
                ServiceError::ReceiveFailed(format!("Failed to deserialize response: {}", e))
            })?;

            // Check for service error
            if let Response::Error { message } = &response {
                return Err(ServiceError::RemoteError(message.clone()));
            }

            Ok(response)
        }
    }

    /// Wait for the service to become available.
    pub async fn wait_for_service(&self, timeout: Duration) -> Result<(), ServiceError> {
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(100);

        while start.elapsed() < timeout {
            #[cfg(unix)]
            let exists = self.socket_path.exists();
            #[cfg(windows)]
            let exists = self.is_service_available();

            if exists {
                match self.connect().await {
                    Ok(()) => return Ok(()),
                    Err(_) => {
                        tokio::time::sleep(poll_interval).await;
                    }
                }
            } else {
                tokio::time::sleep(poll_interval).await;
            }
        }

        Err(ServiceError::Timeout)
    }

    /// Ping the service.
    pub async fn ping(&self) -> Result<(), ServiceError> {
        match self.request(Request::Ping).await? {
            Response::Pong => Ok(()),
            other => Err(ServiceError::RemoteError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Connect to the service, spawning the Tauri app if necessary.
    pub async fn connect_or_spawn(&self) -> Result<(), ServiceError> {
        // First try to just connect
        if self.connect().await.is_ok() {
            return Ok(());
        }

        // Connection failed, try to spawn the app
        Self::spawn_app().map_err(|e| {
            ServiceError::ConnectionFailed(format!("Failed to spawn app: {}", e))
        })?;

        // Wait for service to be ready
        self.wait_for_service(Duration::from_secs(10)).await
    }

    /// Spawn the OmniRec Tauri app in headless mode.
    fn spawn_app() -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            // On macOS, prefer using `open -a OmniRec --args --headless`
            // This uses Launch Services and finds the app bundle correctly
            let result = std::process::Command::new("open")
                .args(["-a", "OmniRec", "--args", "--headless"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();

            if let Ok(status) = result {
                if status.success() {
                    return Ok(());
                }
            }

            // Fall back to binary search if `open` fails
            if let Some(app_path) = Self::find_app_binary() {
                std::process::Command::new(&app_path)
                    .arg("--headless")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .map_err(|e| format!("Failed to spawn app: {}", e))?;
                return Ok(());
            }

            Err("OmniRec app not found".to_string())
        }

        #[cfg(target_os = "linux")]
        {
            if let Some(app_path) = Self::find_app_binary() {
                std::process::Command::new(&app_path)
                    .arg("--headless")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .map_err(|e| format!("Failed to spawn app: {}", e))?;
                return Ok(());
            }
            Err("omnirec binary not found".to_string())
        }

        #[cfg(target_os = "windows")]
        {
            if let Some(app_path) = Self::find_app_binary() {
                std::process::Command::new(&app_path)
                    .arg("--headless")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .map_err(|e| format!("Failed to spawn app: {}", e))?;
                return Ok(());
            }
            Err("omnirec.exe not found".to_string())
        }
    }

    /// Find the OmniRec app binary path.
    fn find_app_binary() -> Option<std::path::PathBuf> {
        #[cfg(target_os = "macos")]
        {
            // The Tauri app binary is named "omnirec" (same as CLI, but in different locations)
            // 1. macOS .app bundle paths (inside the bundle, the binary is named "omnirec")
            let bundle_paths = [
                "/Applications/OmniRec.app/Contents/MacOS/omnirec",
                "~/Applications/OmniRec.app/Contents/MacOS/omnirec",
            ];
            for path in &bundle_paths {
                let path = shellexpand::tilde(path).into_owned();
                let path = std::path::PathBuf::from(path);
                if path.exists() {
                    return Some(path);
                }
            }

            // 2. Development build - look in src-tauri/target/
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(workspace_root) = exe_path.parent().and_then(|p| p.parent()) {
                    // Check src-tauri/target/release/omnirec or src-tauri/target/debug/omnirec
                    for target_dir in ["release", "debug"] {
                        let path = workspace_root.join("src-tauri").join("target").join(target_dir).join("omnirec");
                        if path.exists() {
                            return Some(path);
                        }
                    }
                }
            }

            // 3. In PATH (but exclude ourselves - the CLI binary)
            if let Ok(path) = which::which("omnirec") {
                // Make sure it's not the CLI binary
                if let Ok(current_exe) = std::env::current_exe() {
                    if path != current_exe {
                        return Some(path);
                    }
                }
            }

            None
        }

        #[cfg(target_os = "linux")]
        {
            // The Tauri app binary is named "omnirec" (same as CLI, but in different locations)
            // 1. Development build - look in src-tauri/target/
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(workspace_root) = exe_path.parent().and_then(|p| p.parent()) {
                    for target_dir in ["release", "debug"] {
                        let path = workspace_root.join("src-tauri").join("target").join(target_dir).join("omnirec");
                        if path.exists() {
                            return Some(path);
                        }
                    }
                }
            }

            // 2. In PATH (but exclude ourselves - the CLI binary)
            if let Ok(path) = which::which("omnirec") {
                if let Ok(current_exe) = std::env::current_exe() {
                    if path != current_exe {
                        return Some(path);
                    }
                }
            }

            // 3. Common installation paths
            let common_paths = ["/usr/bin/omnirec", "/usr/local/bin/omnirec", "/opt/omnirec/bin/omnirec"];
            for path in &common_paths {
                let path = std::path::PathBuf::from(path);
                if path.exists() {
                    // Make sure it's not the CLI binary
                    if let Ok(current_exe) = std::env::current_exe() {
                        if path != current_exe {
                            return Some(path);
                        }
                    }
                }
            }

            None
        }

        #[cfg(target_os = "windows")]
        {
            // The Tauri app binary is named "omnirec.exe" (same as CLI, but in different locations)
            // 1. Development build - look in src-tauri/target/
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(workspace_root) = exe_path.parent().and_then(|p| p.parent()) {
                    for target_dir in ["release", "debug"] {
                        let path = workspace_root.join("src-tauri").join("target").join(target_dir).join("omnirec.exe");
                        if path.exists() {
                            return Some(path);
                        }
                    }
                }
            }

            // 2. In PATH (but exclude ourselves - the CLI binary)
            if let Ok(path) = which::which("omnirec") {
                if let Ok(current_exe) = std::env::current_exe() {
                    if path != current_exe {
                        return Some(path);
                    }
                }
            }

            // 3. Common installation paths
            let common_paths = [
                r"C:\Program Files\OmniRec\omnirec.exe",
                r"C:\Program Files (x86)\OmniRec\omnirec.exe",
            ];
            for path in &common_paths {
                let path = std::path::PathBuf::from(path);
                if path.exists() {
                    // Make sure it's not the CLI binary
                    if let Ok(current_exe) = std::env::current_exe() {
                        if path != current_exe {
                            return Some(path);
                        }
                    }
                }
            }

            None
        }
    }
}

impl Default for ServiceClient {
    fn default() -> Self {
        Self::new()
    }
}
