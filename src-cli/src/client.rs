//! IPC client for communicating with omnirec-service.
//!
//! This is a simplified version of the Tauri client, adapted for CLI use.

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
            socket_path: get_socket_path(),
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

    /// Connect to the service, spawning it if necessary.
    pub async fn connect_or_spawn(&self) -> Result<(), ServiceError> {
        // First try to just connect
        if self.connect().await.is_ok() {
            return Ok(());
        }

        // Connection failed, try to spawn the service
        let service_path = Self::find_service_binary().map_err(|e| {
            ServiceError::ConnectionFailed(format!("Cannot find service binary: {}", e))
        })?;

        std::process::Command::new(&service_path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| {
                ServiceError::ConnectionFailed(format!("Failed to spawn service: {}", e))
            })?;

        // Wait for service to be ready
        self.wait_for_service(Duration::from_secs(10)).await
    }

    /// Find the service binary path.
    fn find_service_binary() -> Result<std::path::PathBuf, String> {
        #[cfg(windows)]
        const SERVICE_BINARY: &str = "omnirec-service.exe";
        #[cfg(not(windows))]
        const SERVICE_BINARY: &str = "omnirec-service";

        // 1. Sibling binary (development or bundled)
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(dir) = exe_path.parent() {
                let path = dir.join(SERVICE_BINARY);
                if path.exists() {
                    return Ok(path);
                }
            }
        }

        // 2. In PATH
        if let Ok(path) = which::which(SERVICE_BINARY) {
            return Ok(path);
        }

        // 3. Common installation paths
        #[cfg(windows)]
        let common_paths = [
            r"C:\Program Files\OmniRec\omnirec-service.exe",
            r"C:\Program Files (x86)\OmniRec\omnirec-service.exe",
        ];
        #[cfg(not(windows))]
        let common_paths = ["/usr/bin/omnirec-service", "/usr/local/bin/omnirec-service"];

        for path in &common_paths {
            let path = std::path::PathBuf::from(path);
            if path.exists() {
                return Ok(path);
            }
        }

        Err(format!("{} binary not found", SERVICE_BINARY))
    }
}

impl Default for ServiceClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the platform-specific socket path for the service.
#[cfg(unix)]
fn get_socket_path() -> std::path::PathBuf {
    #[cfg(target_os = "linux")]
    {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
        std::path::PathBuf::from(runtime_dir)
            .join("omnirec")
            .join("service.sock")
    }

    #[cfg(target_os = "macos")]
    {
        let tmpdir = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
        std::path::PathBuf::from(tmpdir)
            .join("omnirec")
            .join("service.sock")
    }
}
