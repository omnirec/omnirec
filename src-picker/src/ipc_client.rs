//! IPC client for communicating with the main omnirec app.
//!
//! Connects to the Unix socket server in the main app to query the current
//! capture selection when XDPH invokes us.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// IPC message sent from picker to main app.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcRequest {
    /// Query the current capture selection.
    QuerySelection,
}

/// Geometry for region capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// IPC response from main app to picker.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcResponse {
    /// Current capture selection.
    Selection {
        /// Type of source: "monitor", "window", or "region"
        source_type: String,
        /// Source identifier (monitor name or window address)
        source_id: String,
        /// Geometry for region capture (None for monitor/window)
        #[serde(skip_serializing_if = "Option::is_none")]
        geometry: Option<Geometry>,
    },
    /// No selection available.
    NoSelection,
    /// Error occurred.
    Error { message: String },
}

/// Get the IPC socket path.
fn get_socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(runtime_dir)
        .join("omnirec")
        .join("picker.sock")
}

/// Query the main app for the current capture selection.
pub async fn query_selection() -> Result<IpcResponse, String> {
    let socket_path = get_socket_path();

    // Connect to the Unix socket
    let stream = UnixStream::connect(&socket_path).await.map_err(|e| {
        format!(
            "Failed to connect to main app (is it running?): {} (path: {:?})",
            e, socket_path
        )
    })?;

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Send query request
    let request = IpcRequest::QuerySelection;
    let request_json =
        serde_json::to_string(&request).map_err(|e| format!("Failed to serialize request: {}", e))?;

    writer
        .write_all(request_json.as_bytes())
        .await
        .map_err(|e| format!("Failed to write to socket: {}", e))?;
    writer
        .write_all(b"\n")
        .await
        .map_err(|e| format!("Failed to write newline: {}", e))?;
    writer
        .flush()
        .await
        .map_err(|e| format!("Failed to flush socket: {}", e))?;

    // Read response
    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    let response: IpcResponse = serde_json::from_str(response_line.trim())
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_query_selection() {
        let request = IpcRequest::QuerySelection;
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(json, r#"{"type":"query_selection"}"#);
    }

    #[test]
    fn test_deserialize_selection_monitor() {
        let json = r#"{"type":"selection","source_type":"monitor","source_id":"DP-1"}"#;
        let response: IpcResponse = serde_json::from_str(json).unwrap();
        match response {
            IpcResponse::Selection {
                source_type,
                source_id,
                geometry,
            } => {
                assert_eq!(source_type, "monitor");
                assert_eq!(source_id, "DP-1");
                assert!(geometry.is_none());
            }
            _ => panic!("Expected Selection response"),
        }
    }

    #[test]
    fn test_deserialize_selection_region() {
        let json = r#"{"type":"selection","source_type":"region","source_id":"DP-1","geometry":{"x":100,"y":200,"width":800,"height":600}}"#;
        let response: IpcResponse = serde_json::from_str(json).unwrap();
        match response {
            IpcResponse::Selection {
                source_type,
                source_id,
                geometry,
            } => {
                assert_eq!(source_type, "region");
                assert_eq!(source_id, "DP-1");
                let geom = geometry.unwrap();
                assert_eq!(geom.x, 100);
                assert_eq!(geom.y, 200);
                assert_eq!(geom.width, 800);
                assert_eq!(geom.height, 600);
            }
            _ => panic!("Expected Selection response"),
        }
    }

    #[test]
    fn test_deserialize_no_selection() {
        let json = r#"{"type":"no_selection"}"#;
        let response: IpcResponse = serde_json::from_str(json).unwrap();
        assert!(matches!(response, IpcResponse::NoSelection));
    }
}
