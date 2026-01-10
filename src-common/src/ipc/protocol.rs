//! IPC message framing and transport protocol.

use std::path::PathBuf;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Maximum IPC message size (64 KB)
pub const MAX_MESSAGE_SIZE: usize = 65536;

/// Error type for IPC operations.
#[derive(Debug)]
pub enum IpcError {
    /// I/O error during read/write
    Io(std::io::Error),
    /// Message exceeds maximum size
    MessageTooLarge { size: usize, max: usize },
    /// JSON parsing failed
    ParseError(String),
    /// Connection closed
    ConnectionClosed,
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpcError::Io(e) => write!(f, "I/O error: {}", e),
            IpcError::MessageTooLarge { size, max } => {
                write!(f, "Message too large: {} bytes (max {})", size, max)
            }
            IpcError::ParseError(e) => write!(f, "Parse error: {}", e),
            IpcError::ConnectionClosed => write!(f, "Connection closed"),
        }
    }
}

impl std::error::Error for IpcError {}

impl From<std::io::Error> for IpcError {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            IpcError::ConnectionClosed
        } else {
            IpcError::Io(e)
        }
    }
}

/// Get the platform-specific socket path for the IPC connection.
pub fn get_socket_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
        PathBuf::from(runtime_dir)
            .join("omnirec")
            .join("service.sock")
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

/// Read a length-prefixed message with size validation.
///
/// Message format:
/// ```text
/// ┌──────────────────┬─────────────────────────────────┐
/// │ Length (4 bytes) │ JSON Payload (variable length)  │
/// │ Little-endian    │ Max 65,536 bytes                │
/// └──────────────────┴─────────────────────────────────┘
/// ```
pub async fn read_message<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, IpcError> {
    // Read 4-byte length prefix
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;

    // Validate size BEFORE allocating
    if len > MAX_MESSAGE_SIZE {
        return Err(IpcError::MessageTooLarge {
            size: len,
            max: MAX_MESSAGE_SIZE,
        });
    }

    // Read payload
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;

    Ok(buf)
}

/// Write a length-prefixed message.
pub async fn write_message<W: AsyncWrite + Unpin>(
    writer: &mut W,
    data: &[u8],
) -> Result<(), IpcError> {
    if data.len() > MAX_MESSAGE_SIZE {
        return Err(IpcError::MessageTooLarge {
            size: data.len(),
            max: MAX_MESSAGE_SIZE,
        });
    }

    // Write 4-byte length prefix
    let len = data.len() as u32;
    writer.write_all(&len.to_le_bytes()).await?;

    // Write payload
    writer.write_all(data).await?;
    writer.flush().await?;

    Ok(())
}

/// Read and deserialize a JSON message.
pub async fn read_json<R: AsyncRead + Unpin, T: serde::de::DeserializeOwned>(
    reader: &mut R,
) -> Result<T, IpcError> {
    let data = read_message(reader).await?;
    serde_json::from_slice(&data).map_err(|e| IpcError::ParseError(e.to_string()))
}

/// Serialize and write a JSON message.
pub async fn write_json<W: AsyncWrite + Unpin, T: serde::Serialize>(
    writer: &mut W,
    value: &T,
) -> Result<(), IpcError> {
    let data = serde_json::to_vec(value).map_err(|e| IpcError::ParseError(e.to_string()))?;
    write_message(writer, &data).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[tokio::test]
    async fn test_message_roundtrip() {
        let original = b"hello world";
        let mut buf = Vec::new();

        // Write
        write_message(&mut buf, original).await.unwrap();

        // Read
        let mut cursor = Cursor::new(buf);
        let read = read_message(&mut cursor).await.unwrap();

        assert_eq!(read, original);
    }

    #[tokio::test]
    async fn test_message_too_large() {
        let oversized = vec![0u8; MAX_MESSAGE_SIZE + 1];
        let mut buf = Vec::new();

        let result = write_message(&mut buf, &oversized).await;
        assert!(matches!(result, Err(IpcError::MessageTooLarge { .. })));
    }
}
