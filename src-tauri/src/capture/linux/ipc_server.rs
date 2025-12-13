//! IPC server for communicating with the picker service.
//!
//! This module runs a Unix socket server that the picker service connects to
//! when it receives portal requests. The server responds with the current
//! capture selection stored in app state.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::RwLock;

/// Geometry for region capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The current capture selection stored by the main app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSelection {
    /// Type of source: "monitor", "window", or "region"
    pub source_type: String,
    /// Source identifier (monitor name or window address)
    pub source_id: String,
    /// Geometry for region capture (None for monitor/window)
    pub geometry: Option<Geometry>,
}

/// IPC message sent from picker to main app.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcRequest {
    /// Query the current capture selection.
    QuerySelection,
}

/// IPC response from main app to picker.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcResponse {
    /// Current capture selection.
    Selection {
        source_type: String,
        source_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        geometry: Option<Geometry>,
    },
    /// No selection available.
    NoSelection,
    /// Error occurred.
    Error { message: String },
}

/// Shared state for the IPC server.
pub struct IpcServerState {
    /// Current capture selection (set by UI before recording starts)
    pub selection: Option<CaptureSelection>,
}

impl Default for IpcServerState {
    fn default() -> Self {
        Self { selection: None }
    }
}

/// Get the IPC socket path.
pub fn get_socket_path() -> PathBuf {
    let runtime_dir =
        std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(runtime_dir)
        .join("omnirec")
        .join("picker.sock")
}

/// Handle a single client connection.
async fn handle_client(
    stream: UnixStream,
    state: Arc<RwLock<IpcServerState>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // Read request
    reader.read_line(&mut line).await?;
    let line = line.trim();

    if line.is_empty() {
        return Ok(());
    }

    // Parse request
    let request: IpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            let response = IpcResponse::Error {
                message: format!("Invalid request: {}", e),
            };
            let response_json = serde_json::to_string(&response)?;
            writer.write_all(response_json.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            return Ok(());
        }
    };

    // Handle request
    let response = match request {
        IpcRequest::QuerySelection => {
            let state = state.read().await;
            match &state.selection {
                Some(sel) => {
                    eprintln!("[IPC] Picker queried selection: type={}, id={}, geometry={:?}",
                        sel.source_type, sel.source_id, sel.geometry);
                    IpcResponse::Selection {
                        source_type: sel.source_type.clone(),
                        source_id: sel.source_id.clone(),
                        geometry: sel.geometry.clone(),
                    }
                }
                None => {
                    eprintln!("[IPC] Picker queried but no selection available");
                    IpcResponse::NoSelection
                }
            }
        }
    };

    // Send response
    let response_json = serde_json::to_string(&response)?;
    writer.write_all(response_json.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;

    Ok(())
}

/// Start the IPC server.
///
/// Returns a handle to the server state that can be used to update the selection.
pub async fn start_ipc_server() -> Result<Arc<RwLock<IpcServerState>>, Box<dyn std::error::Error + Send + Sync>>
{
    let socket_path = get_socket_path();

    // Create parent directory if needed
    if let Some(parent) = socket_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Remove existing socket file
    let _ = tokio::fs::remove_file(&socket_path).await;

    // Bind to socket
    let listener = UnixListener::bind(&socket_path)?;

    let state = Arc::new(RwLock::new(IpcServerState::default()));
    let state_clone = state.clone();

    // Spawn server task
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let state = state_clone.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(stream, state).await {
                            eprintln!("IPC client error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("IPC accept error: {}", e);
                }
            }
        }
    });

    Ok(state)
}

/// Update the current capture selection.
#[allow(dead_code)]
pub async fn set_selection(
    state: &Arc<RwLock<IpcServerState>>,
    selection: CaptureSelection,
) {
    eprintln!("[IPC] Setting selection: type={}, id={}, geometry={:?}", 
        selection.source_type, selection.source_id, selection.geometry);
    let mut state = state.write().await;
    state.selection = Some(selection);
}

/// Clear the current capture selection.
#[allow(dead_code)]
pub async fn clear_selection(state: &Arc<RwLock<IpcServerState>>) {
    let mut state = state.write().await;
    state.selection = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_selection_response() {
        let response = IpcResponse::Selection {
            source_type: "monitor".to_string(),
            source_id: "DP-1".to_string(),
            geometry: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains(r#""type":"selection""#));
        assert!(json.contains(r#""source_type":"monitor""#));
        assert!(json.contains(r#""source_id":"DP-1""#));
    }

    #[test]
    fn test_serialize_region_selection() {
        let response = IpcResponse::Selection {
            source_type: "region".to_string(),
            source_id: "DP-1".to_string(),
            geometry: Some(Geometry {
                x: 100,
                y: 200,
                width: 800,
                height: 600,
            }),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains(r#""source_type":"region""#));
        assert!(json.contains(r#""geometry""#));
        assert!(json.contains(r#""width":800"#));
    }

    #[test]
    fn test_deserialize_query_request() {
        let json = r#"{"type":"query_selection"}"#;
        let request: IpcRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(request, IpcRequest::QuerySelection));
    }
}
