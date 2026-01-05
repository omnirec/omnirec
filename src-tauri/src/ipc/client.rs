//! IPC client implementation for communicating with omnirec-service.

use omnirec_common::ipc::{Request, Response, MAX_MESSAGE_SIZE};
use std::io::{Read, Write};
use std::time::Duration;
use tokio::sync::Mutex;

/// Error type for service client operations.
#[derive(Debug, Clone)]
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
    ServiceError(String),
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
            ServiceError::ServiceError(msg) => write!(f, "Service error: {}", msg),
            ServiceError::Timeout => write!(f, "Request timed out"),
        }
    }
}

impl std::error::Error for ServiceError {}

/// Connection state for the service client.
enum ConnectionState {
    Disconnected,
    #[cfg(unix)]
    Connected(std::os::unix::net::UnixStream),
    #[cfg(windows)]
    Connected(std::fs::File),
}

/// Client for communicating with the OmniRec service.
///
/// The client maintains a persistent connection to the service and provides
/// methods for sending requests and receiving responses.
pub struct ServiceClient {
    connection: Mutex<ConnectionState>,
    socket_path: std::path::PathBuf,
}

impl ServiceClient {
    /// Create a new service client.
    pub fn new() -> Self {
        Self {
            connection: Mutex::new(ConnectionState::Disconnected),
            socket_path: super::get_socket_path(),
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
            stream
                .set_read_timeout(Some(Duration::from_secs(30)))
                .ok();
            stream
                .set_write_timeout(Some(Duration::from_secs(10)))
                .ok();

            *conn = ConnectionState::Connected(stream);
            tracing::info!("Connected to service at {}", self.socket_path.display());
            Ok(())
        }

        #[cfg(windows)]
        {
            use std::fs::OpenOptions;

            // Named pipe path
            let pipe_path = r"\\.\pipe\omnirec-service";

            // Open the named pipe for read/write
            // FILE_FLAG_OVERLAPPED is not needed for sync I/O
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
            tracing::info!("Connected to service at {}", pipe_path);
            Ok(())
        }
    }

    /// Disconnect from the service.
    pub async fn disconnect(&self) {
        let mut conn = self.connection.lock().await;
        *conn = ConnectionState::Disconnected;
        tracing::info!("Disconnected from service");
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
            stream.write_all(&len.to_le_bytes()).map_err(|e| {
                ServiceError::SendFailed(format!("Failed to write length: {}", e))
            })?;
            stream.write_all(&request_json).map_err(|e| {
                ServiceError::SendFailed(format!("Failed to write request: {}", e))
            })?;
            stream.flush().map_err(|e| {
                ServiceError::SendFailed(format!("Failed to flush: {}", e))
            })?;

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
                return Err(ServiceError::ServiceError(message.clone()));
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
            file.write_all(&len.to_le_bytes()).map_err(|e| {
                ServiceError::SendFailed(format!("Failed to write length: {}", e))
            })?;
            file.write_all(&request_json).map_err(|e| {
                ServiceError::SendFailed(format!("Failed to write request: {}", e))
            })?;
            file.flush().map_err(|e| {
                ServiceError::SendFailed(format!("Failed to flush: {}", e))
            })?;

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
                return Err(ServiceError::ServiceError(message.clone()));
            }

            Ok(response)
        }
    }

    /// Check if the service is available (socket/pipe exists).
    pub fn is_service_available(&self) -> bool {
        #[cfg(unix)]
        {
            self.socket_path.exists()
        }

        #[cfg(windows)]
        {
            // On Windows, try to open the named pipe to check if it exists
            use std::fs::OpenOptions;
            let pipe_path = r"\\.\pipe\omnirec-service";
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(pipe_path)
                .is_ok()
        }
    }

    /// Wait for the service to become available.
    pub async fn wait_for_service(&self, timeout: Duration) -> Result<(), ServiceError> {
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(100);

        while start.elapsed() < timeout {
            if self.socket_path.exists() {
                // Try to connect to verify service is ready
                match self.connect().await {
                    Ok(()) => return Ok(()),
                    Err(_) => {
                        // Socket exists but connection failed, keep trying
                        tokio::time::sleep(poll_interval).await;
                    }
                }
            } else {
                tokio::time::sleep(poll_interval).await;
            }
        }

        Err(ServiceError::Timeout)
    }
}

impl Default for ServiceClient {
    fn default() -> Self {
        Self::new()
    }
}

// Convenience methods for common requests
impl ServiceClient {
    /// List all capturable windows.
    pub async fn list_windows(&self) -> Result<Vec<omnirec_common::WindowInfo>, ServiceError> {
        match self.request(Request::ListWindows).await? {
            Response::Windows { windows } => Ok(windows),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// List all monitors.
    pub async fn list_monitors(&self) -> Result<Vec<omnirec_common::MonitorInfo>, ServiceError> {
        match self.request(Request::ListMonitors).await? {
            Response::Monitors { monitors } => Ok(monitors),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// List all audio sources.
    pub async fn list_audio_sources(
        &self,
    ) -> Result<Vec<omnirec_common::AudioSource>, ServiceError> {
        match self.request(Request::ListAudioSources).await? {
            Response::AudioSources { sources } => Ok(sources),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Get the current recording state.
    pub async fn get_recording_state(&self) -> Result<omnirec_common::RecordingState, ServiceError> {
        match self.request(Request::GetRecordingState).await? {
            Response::RecordingState { state } => Ok(state),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Get elapsed recording time in seconds.
    pub async fn get_elapsed_time(&self) -> Result<u64, ServiceError> {
        match self.request(Request::GetElapsedTime).await? {
            Response::ElapsedTime { seconds } => Ok(seconds),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Start window capture.
    pub async fn start_window_capture(&self, window_handle: isize) -> Result<(), ServiceError> {
        match self
            .request(Request::StartWindowCapture { window_handle })
            .await?
        {
            Response::RecordingStarted => Ok(()),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Start display capture.
    pub async fn start_display_capture(
        &self,
        monitor_id: String,
        width: u32,
        height: u32,
    ) -> Result<(), ServiceError> {
        match self
            .request(Request::StartDisplayCapture {
                monitor_id,
                width,
                height,
            })
            .await?
        {
            Response::RecordingStarted => Ok(()),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Start region capture.
    pub async fn start_region_capture(
        &self,
        monitor_id: String,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<(), ServiceError> {
        match self
            .request(Request::StartRegionCapture {
                monitor_id,
                x,
                y,
                width,
                height,
            })
            .await?
        {
            Response::RecordingStarted => Ok(()),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Start portal-based capture (GNOME/KDE tray mode).
    pub async fn start_portal_capture(&self) -> Result<(), ServiceError> {
        match self.request(Request::StartPortalCapture).await? {
            Response::RecordingStarted => Ok(()),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Stop recording and get the output file paths.
    pub async fn stop_recording(&self) -> Result<(String, String), ServiceError> {
        match self.request(Request::StopRecording).await? {
            Response::RecordingStopped {
                file_path,
                source_path,
            } => Ok((file_path, source_path)),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Get the current audio configuration.
    pub async fn get_audio_config(&self) -> Result<omnirec_common::AudioConfig, ServiceError> {
        match self.request(Request::GetAudioConfig).await? {
            Response::AudioConfig(config) => Ok(config),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Set the audio configuration.
    pub async fn set_audio_config(
        &self,
        enabled: bool,
        source_id: Option<String>,
        microphone_id: Option<String>,
        echo_cancellation: bool,
    ) -> Result<(), ServiceError> {
        match self
            .request(Request::SetAudioConfig {
                enabled,
                source_id,
                microphone_id,
                echo_cancellation,
            })
            .await?
        {
            Response::Ok => Ok(()),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Get a window thumbnail.
    pub async fn get_window_thumbnail(
        &self,
        window_handle: isize,
    ) -> Result<(String, u32, u32), ServiceError> {
        match self
            .request(Request::GetWindowThumbnail { window_handle })
            .await?
        {
            Response::Thumbnail {
                data,
                width,
                height,
            } => Ok((data, width, height)),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Get a display thumbnail.
    pub async fn get_display_thumbnail(
        &self,
        monitor_id: String,
    ) -> Result<(String, u32, u32), ServiceError> {
        match self
            .request(Request::GetDisplayThumbnail { monitor_id })
            .await?
        {
            Response::Thumbnail {
                data,
                width,
                height,
            } => Ok((data, width, height)),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Get a region preview.
    pub async fn get_region_preview(
        &self,
        monitor_id: String,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<(String, u32, u32), ServiceError> {
        match self
            .request(Request::GetRegionPreview {
                monitor_id,
                x,
                y,
                width,
                height,
            })
            .await?
        {
            Response::Thumbnail {
                data,
                width,
                height,
            } => Ok((data, width, height)),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Show a highlight around a display area.
    pub async fn show_display_highlight(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<(), ServiceError> {
        match self
            .request(Request::ShowDisplayHighlight {
                x,
                y,
                width,
                height,
            })
            .await?
        {
            Response::Ok => Ok(()),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Show a highlight around a window.
    pub async fn show_window_highlight(&self, window_handle: isize) -> Result<(), ServiceError> {
        match self
            .request(Request::ShowWindowHighlight { window_handle })
            .await?
        {
            Response::Ok => Ok(()),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Ping the service.
    pub async fn ping(&self) -> Result<(), ServiceError> {
        match self.request(Request::Ping).await? {
            Response::Pong => Ok(()),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Request service shutdown.
    pub async fn shutdown(&self) -> Result<(), ServiceError> {
        match self.request(Request::Shutdown).await? {
            Response::Ok => {
                // Disconnect after shutdown request
                self.disconnect().await;
                Ok(())
            }
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Get transcription configuration.
    pub async fn get_transcription_config(
        &self,
    ) -> Result<omnirec_common::TranscriptionConfig, ServiceError> {
        match self.request(Request::GetTranscriptionConfig).await? {
            Response::TranscriptionConfig(config) => Ok(config),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Set transcription configuration.
    pub async fn set_transcription_config(&self, enabled: bool) -> Result<(), ServiceError> {
        match self
            .request(Request::SetTranscriptionConfig { enabled })
            .await?
        {
            Response::Ok => Ok(()),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Get transcription status.
    pub async fn get_transcription_status(
        &self,
    ) -> Result<omnirec_common::TranscriptionStatus, ServiceError> {
        match self.request(Request::GetTranscriptionStatus).await? {
            Response::TranscriptionStatus(status) => Ok(status),
            other => Err(ServiceError::ServiceError(format!(
                "Unexpected response: {:?}",
                other
            ))),
        }
    }

    /// Ensure the service is connected, reconnecting if necessary.
    ///
    /// This method checks if the connection is still valid by sending a ping.
    /// If the ping fails, it attempts to reconnect.
    pub async fn ensure_connected(&self) -> Result<(), ServiceError> {
        // First check if we think we're connected
        if self.is_connected().await {
            // Try a ping to verify the connection is still alive
            match self.ping().await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    tracing::warn!("Connection lost, will reconnect: {}", e);
                    self.disconnect().await;
                }
            }
        }

        // Not connected or ping failed, try to connect
        self.connect().await
    }

    /// Reconnect to the service, spawning it if necessary.
    ///
    /// This is useful when the service has crashed or was stopped externally.
    pub async fn reconnect_or_spawn(&self) -> Result<(), ServiceError> {
        // First try to just connect
        if self.connect().await.is_ok() {
            return Ok(());
        }

        // Connection failed, try to spawn the service
        tracing::info!("Service not available, attempting to spawn...");

        // Find and spawn the service binary
        let service_path = Self::find_service_binary().map_err(|e| {
            ServiceError::ConnectionFailed(format!("Cannot find service binary: {}", e))
        })?;

        std::process::Command::new(&service_path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(|e| ServiceError::ConnectionFailed(format!("Failed to spawn service: {}", e)))?;

        // Wait for service to be ready
        self.wait_for_service(std::time::Duration::from_secs(10))
            .await
    }

    /// Find the service binary path.
    fn find_service_binary() -> Result<std::path::PathBuf, String> {
        // Service binary name (with .exe on Windows)
        #[cfg(windows)]
        const SERVICE_BINARY: &str = "omnirec-service.exe";
        #[cfg(not(windows))]
        const SERVICE_BINARY: &str = "omnirec-service";

        // Try multiple locations in order of preference:

        // 1. In development: sibling binary in target directory
        if let Ok(exe_path) = std::env::current_exe() {
            let dev_path = exe_path.parent().map(|p| p.join(SERVICE_BINARY));
            if let Some(path) = dev_path {
                if path.exists() {
                    return Ok(path);
                }
            }
        }

        // 2. Bundled alongside the main binary (production)
        if let Ok(exe_path) = std::env::current_exe() {
            // Same directory as main binary
            let bundled_path = exe_path.parent().map(|p| p.join(SERVICE_BINARY));
            if let Some(path) = bundled_path {
                if path.exists() {
                    return Ok(path);
                }
            }

            // On macOS bundle: Contents/MacOS/omnirec-service
            #[cfg(target_os = "macos")]
            {
                let macos_path = exe_path
                    .parent()
                    .and_then(|p| p.parent())
                    .map(|p| p.join("MacOS").join(SERVICE_BINARY));
                if let Some(path) = macos_path {
                    if path.exists() {
                        return Ok(path);
                    }
                }
            }
        }

        // 3. In PATH
        if let Ok(path) = which::which(SERVICE_BINARY) {
            return Ok(path);
        }

        // 4. Common installation paths
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
