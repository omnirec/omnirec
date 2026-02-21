//! IPC server with secure socket setup and peer verification.

use omnirec_common::ipc::{read_json, write_json, Request, Response};
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{error, info, warn};

#[cfg(unix)]
use omnirec_common::ipc::get_socket_path;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::path::Path;

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
        warn!("Socket mode is {:o}, expected {:o}", actual, SOCKET_MODE);
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

/// Named pipe name for Windows IPC.
#[cfg(windows)]
const PIPE_NAME: &str = r"\\.\pipe\omnirec-service";

/// Create a security descriptor that only allows the current user.
///
/// This builds an SDDL string dynamically using the current user's SID,
/// which provides equivalent security to Unix socket permissions (0600).
#[cfg(windows)]
fn create_security_attributes() -> Result<
    (
        windows::Win32::Security::SECURITY_ATTRIBUTES,
        Vec<u8>, // Keep the descriptor alive
    ),
    Box<dyn std::error::Error>,
> {
    use std::ptr::null_mut;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE};
    use windows::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1,
    };
    use windows::Win32::Security::{
        GetSecurityDescriptorLength, GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR,
        SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    // Get the current user's SID
    let user_sid_string = unsafe {
        // Open process token
        let mut token_handle = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle)?;

        // Get token user info size
        let mut token_info_len: u32 = 0;
        let _ = GetTokenInformation(token_handle, TokenUser, None, 0, &mut token_info_len);

        // Allocate buffer and get token user info
        let mut token_info = vec![0u8; token_info_len as usize];
        GetTokenInformation(
            token_handle,
            TokenUser,
            Some(token_info.as_mut_ptr() as *mut _),
            token_info_len,
            &mut token_info_len,
        )?;

        let _ = CloseHandle(token_handle);

        // Extract the SID from TOKEN_USER structure
        let token_user = &*(token_info.as_ptr() as *const TOKEN_USER);
        let sid = token_user.User.Sid;

        // Convert SID to string
        let mut sid_string_ptr = windows::core::PWSTR::null();
        ConvertSidToStringSidW(sid, &mut sid_string_ptr)?;

        // Copy the string and free the original
        let sid_string = sid_string_ptr.to_string()?;
        let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(
            sid_string_ptr.0 as *mut _,
        )));

        sid_string
    };

    // Build SDDL: D:(A;;GA;;;<user-sid>) = Allow Generic All to current user only
    let sddl = format!("D:(A;;GA;;;{})\0", user_sid_string);
    let sddl_wide: Vec<u16> = sddl.encode_utf16().collect();

    let mut sd: PSECURITY_DESCRIPTOR = PSECURITY_DESCRIPTOR(null_mut());

    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl_wide.as_ptr()),
            SDDL_REVISION_1,
            &mut sd,
            None,
        )?;
    }

    // We need to keep the security descriptor alive, so copy it
    // The descriptor is variable-length, so we'll store it as bytes
    let sd_ptr = sd.0 as *const u8;
    let sd_size = unsafe { GetSecurityDescriptorLength(sd) as usize };
    let sd_bytes = unsafe { std::slice::from_raw_parts(sd_ptr, sd_size).to_vec() };

    // Free the original (we've copied it)
    unsafe {
        let _ = LocalFree(Some(windows::Win32::Foundation::HLOCAL(sd.0)));
    }

    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd_bytes.as_ptr() as *mut _,
        bInheritHandle: false.into(),
    };

    info!(
        "Created named pipe with security for user: {}",
        user_sid_string
    );

    Ok((sa, sd_bytes))
}

/// Create a new named pipe server with security attributes.
#[cfg(windows)]
fn create_pipe_server(
    first_instance: bool,
    sa: &mut windows::Win32::Security::SECURITY_ATTRIBUTES,
) -> Result<tokio::net::windows::named_pipe::NamedPipeServer, std::io::Error> {
    use tokio::net::windows::named_pipe::{PipeMode, ServerOptions};
    
    unsafe {
        ServerOptions::new()
            .first_pipe_instance(first_instance)
            .pipe_mode(PipeMode::Byte)
            .create_with_security_attributes_raw(
                PIPE_NAME,
                sa as *mut _ as *mut std::ffi::c_void,
            )
    }
}

/// Run the IPC server (Windows implementation).
#[cfg(windows)]
pub async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    use omnirec_common::security::peer_verify::verify_peer;
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;

    info!("Starting IPC server at {}", PIPE_NAME);

    info!("IPC server listening on {}", PIPE_NAME);

    // Create the first pipe instance
    let mut server = {
        let (mut sa, _sd_bytes) = create_security_attributes()?;
        create_pipe_server(true, &mut sa)?
    };

    loop {
        // Check for shutdown before accepting new connections
        if crate::is_shutdown_requested() {
            info!("Shutdown requested, stopping IPC server");
            break;
        }

        // Wait for a client to connect with a timeout
        let connect_result = tokio::select! {
            result = server.connect() => Some(result),
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => None,
        };

        match connect_result {
            Some(Ok(())) => {
                // Client connected - get the raw handle for peer verification
                let raw_handle = server.as_raw_handle();
                let handle = HANDLE(raw_handle);

                // Verify peer before processing requests
                match verify_peer(handle) {
                    Ok(peer) => {
                        info!(
                            "Authenticated peer: pid={} exe={:?}",
                            peer.pid, peer.executable
                        );

                        // Take ownership of the connected pipe and create a new one for the next client
                        let connected_pipe = server;
                        server = {
                            let (mut sa, _sd_bytes) = create_security_attributes()?;
                            create_pipe_server(false, &mut sa)?
                        };

                        let peer_info = format!("pid={}", peer.pid);
                        tokio::spawn(handle_client(connected_pipe, peer_info));
                    }
                    Err(e) => {
                        warn!("Rejected connection: {:?}", e);
                        // Disconnect the client and create a new pipe for the next connection
                        server.disconnect()?;
                    }
                }
            }
            Some(Err(e)) => {
                error!("Accept error: {}", e);
                // Try to recreate the pipe
                server = {
                    let (mut sa, _sd_bytes) = create_security_attributes()?;
                    create_pipe_server(false, &mut sa)?
                };
            }
            None => {
                // Timeout, check shutdown flag and continue
                continue;
            }
        }
    }

    Ok(())
}
