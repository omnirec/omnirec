//! IPC server with secure socket setup and peer verification.

use omnirec_common::ipc::{get_socket_path, read_json, write_json, Request, Response};
use std::path::Path;
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{error, info, warn};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::handlers::handle_request;

/// Socket file permissions (owner read/write only)
#[cfg(unix)]
const SOCKET_MODE: u32 = 0o600;

/// Socket directory permissions (owner read/write/execute only)
#[cfg(unix)]
const DIRECTORY_MODE: u32 = 0o700;

/// Create socket directory with secure permissions.
#[cfg(unix)]
fn create_secure_socket_dir(socket_path: &Path) -> std::io::Result<()> {
    let socket_dir = socket_path
        .parent()
        .expect("Socket must have parent directory");

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

/// Set socket file permissions after binding.
#[cfg(unix)]
fn secure_socket_file(socket_path: &Path) -> std::io::Result<()> {
    std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(SOCKET_MODE))?;

    // Verify permissions were set
    let actual = std::fs::metadata(socket_path)?.permissions().mode() & 0o777;
    if actual != SOCKET_MODE {
        warn!(
            "Socket mode is {:o}, expected {:o}",
            actual, SOCKET_MODE
        );
    }

    Ok(())
}

/// Handle a single authenticated client connection.
async fn handle_client<S>(mut stream: S, peer_info: String)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    info!("Client connected: {}", peer_info);

    loop {
        // Read request
        let request: Request = match read_json(&mut stream).await {
            Ok(req) => req,
            Err(omnirec_common::ipc::IpcError::ConnectionClosed) => {
                info!("Client disconnected: {}", peer_info);
                break;
            }
            Err(e) => {
                error!("Error reading request from {}: {}", peer_info, e);
                break;
            }
        };

        // Validate request parameters
        if let Err(e) = request.validate() {
            warn!("Invalid request from {}: {}", peer_info, e);
            let response = Response::error(format!("Invalid request: {}", e));
            if let Err(e) = write_json(&mut stream, &response).await {
                error!("Error writing response: {}", e);
                break;
            }
            continue;
        }

        // Handle request
        let response = handle_request(request).await;

        // Write response
        if let Err(e) = write_json(&mut stream, &response).await {
            error!("Error writing response to {}: {}", peer_info, e);
            break;
        }
    }
}

/// Run the IPC server (Unix implementation).
#[cfg(unix)]
pub async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    use omnirec_common::security::peer_verify::verify_peer;
    use tokio::net::UnixListener;

    let socket_path = get_socket_path();
    info!("Starting IPC server at {:?}", socket_path);

    // Create secure socket directory
    create_secure_socket_dir(&socket_path)?;

    // Bind socket
    let listener = UnixListener::bind(&socket_path)?;

    // Set socket permissions AFTER binding
    secure_socket_file(&socket_path)?;

    info!("IPC server listening on {:?}", socket_path);

    loop {
        // Check for shutdown before accepting new connections
        if crate::is_shutdown_requested() {
            info!("Shutdown requested, stopping IPC server");
            break;
        }

        // Use select to allow checking shutdown flag periodically
        let accept_result = tokio::select! {
            result = listener.accept() => Some(result),
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => None,
        };

        let (stream, _) = match accept_result {
            Some(Ok(conn)) => conn,
            Some(Err(e)) => {
                error!("Accept error: {}", e);
                continue;
            }
            None => {
                // Timeout, check shutdown flag and continue
                continue;
            }
        };

        // CRITICAL: Verify peer BEFORE any request processing
        let std_stream = stream.into_std()?;
        match verify_peer(&std_stream) {
            Ok(peer) => {
                info!(
                    "Authenticated peer: pid={} exe={:?}",
                    peer.pid, peer.executable
                );
                let stream = tokio::net::UnixStream::from_std(std_stream)?;
                let peer_info = format!("pid={}", peer.pid);
                tokio::spawn(handle_client(stream, peer_info));
            }
            Err(e) => {
                warn!("Rejected connection: {:?}", e);
                // Connection dropped - stream goes out of scope
            }
        }
    }

    Ok(())
}

/// Run the IPC server (Windows implementation).
#[cfg(windows)]
pub async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement Windows named pipe server
    // For now, use a placeholder that compiles
    let socket_path = get_socket_path();
    info!("Starting IPC server at {:?}", socket_path);
    
    // Windows implementation will use named pipes
    // This is a stub for initial compilation
    error!("Windows IPC server not yet implemented");
    std::process::exit(1);
}
