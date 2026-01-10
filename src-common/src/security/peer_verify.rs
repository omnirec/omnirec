//! Peer credential verification for IPC connections.
//!
//! This module provides platform-specific mechanisms to verify that
//! connecting processes are trusted OmniRec binaries.

use std::path::{Path, PathBuf};

use super::{TRUSTED_DIRECTORIES, TRUSTED_EXECUTABLES};

/// Information about a verified peer process.
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Process ID of the peer
    pub pid: i32,
    /// Path to the peer's executable
    pub executable: PathBuf,
}

/// Errors that can occur during peer verification.
#[derive(Debug)]
pub enum PeerVerifyError {
    /// Failed to retrieve peer credentials from socket
    CredentialsFailed(String),
    /// Process with given PID not found
    ProcessNotFound(i32),
    /// Executable is not in the trusted list
    UntrustedExecutable(PathBuf),
    /// Executable is not in a trusted directory
    UntrustedDirectory(PathBuf),
    /// Peer UID doesn't match current user (Unix only)
    #[cfg(unix)]
    UidMismatch { peer: u32, current: u32 },
}

impl std::fmt::Display for PeerVerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeerVerifyError::CredentialsFailed(e) => {
                write!(f, "Failed to get peer credentials: {}", e)
            }
            PeerVerifyError::ProcessNotFound(pid) => write!(f, "Process {} not found", pid),
            PeerVerifyError::UntrustedExecutable(path) => {
                write!(f, "Untrusted executable: {:?}", path)
            }
            PeerVerifyError::UntrustedDirectory(path) => {
                write!(f, "Executable not in trusted directory: {:?}", path)
            }
            #[cfg(unix)]
            PeerVerifyError::UidMismatch { peer, current } => {
                write!(f, "UID mismatch: peer={}, current={}", peer, current)
            }
        }
    }
}

impl std::error::Error for PeerVerifyError {}

/// Verify that an executable path is trusted.
///
/// The executable must:
/// 1. Have a filename matching one of TRUSTED_EXECUTABLES
/// 2. Be located in a TRUSTED_DIRECTORY OR the same directory as the service
fn verify_executable(exe_path: &Path) -> Result<(), PeerVerifyError> {
    // Get executable name
    let exe_name = exe_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| PeerVerifyError::UntrustedExecutable(exe_path.to_path_buf()))?;

    // Check name (handle .exe suffix on Windows)
    #[cfg(windows)]
    let name_matches = TRUSTED_EXECUTABLES.iter().any(|t| {
        exe_name.eq_ignore_ascii_case(t) || exe_name.eq_ignore_ascii_case(&format!("{}.exe", t))
    });

    #[cfg(not(windows))]
    let name_matches = TRUSTED_EXECUTABLES.contains(&exe_name);

    if !name_matches {
        return Err(PeerVerifyError::UntrustedExecutable(exe_path.to_path_buf()));
    }

    // Get executable directory
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| PeerVerifyError::UntrustedDirectory(exe_path.to_path_buf()))?;

    // Check if in trusted directory OR same directory as self
    let self_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    let in_trusted_dir = TRUSTED_DIRECTORIES.iter().any(|d| exe_dir == Path::new(d));
    let same_as_self = self_dir.as_ref().map(|d| exe_dir == d).unwrap_or(false);

    // During development, also allow cargo target directories and CMake build directories
    let exe_dir_str = exe_dir.to_string_lossy();
    let in_target_dir = exe_dir_str.contains("target")
        && (exe_dir_str.contains("debug") || exe_dir_str.contains("release"));
    let in_cmake_build = exe_dir_str.contains("src-picker") && exe_dir_str.contains("build");

    if !in_trusted_dir && !same_as_self && !in_target_dir && !in_cmake_build {
        return Err(PeerVerifyError::UntrustedDirectory(exe_path.to_path_buf()));
    }

    Ok(())
}

/// Verify connecting peer on Linux using SO_PEERCRED.
#[cfg(target_os = "linux")]
pub fn verify_peer(stream: &std::os::unix::net::UnixStream) -> Result<PeerInfo, PeerVerifyError> {
    use std::os::unix::io::AsRawFd;

    let fd = stream.as_raw_fd();

    // Get peer credentials via SO_PEERCRED
    let creds = unsafe {
        let mut creds: libc::ucred = std::mem::zeroed();
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;

        if libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut creds as *mut _ as *mut _,
            &mut len,
        ) != 0
        {
            return Err(PeerVerifyError::CredentialsFailed(
                std::io::Error::last_os_error().to_string(),
            ));
        }
        creds
    };

    // Verify UID matches current user
    let current_uid = unsafe { libc::getuid() };
    if creds.uid != current_uid {
        return Err(PeerVerifyError::UidMismatch {
            peer: creds.uid,
            current: current_uid,
        });
    }

    // Get executable path from /proc
    let exe_path = std::fs::read_link(format!("/proc/{}/exe", creds.pid))
        .map_err(|_| PeerVerifyError::ProcessNotFound(creds.pid))?;

    // Verify executable is trusted
    verify_executable(&exe_path)?;

    Ok(PeerInfo {
        pid: creds.pid,
        executable: exe_path,
    })
}

/// Verify connecting peer on macOS using LOCAL_PEERPID.
#[cfg(target_os = "macos")]
pub fn verify_peer(stream: &std::os::unix::net::UnixStream) -> Result<PeerInfo, PeerVerifyError> {
    use std::os::unix::io::AsRawFd;

    let fd = stream.as_raw_fd();

    // macOS constants not in libc
    const SOL_LOCAL: libc::c_int = 0;
    const LOCAL_PEERPID: libc::c_int = 0x002;

    // Get PID via LOCAL_PEERPID
    let pid = unsafe {
        let mut pid: libc::pid_t = 0;
        let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;

        if libc::getsockopt(
            fd,
            SOL_LOCAL,
            LOCAL_PEERPID,
            &mut pid as *mut _ as *mut _,
            &mut len,
        ) != 0
        {
            return Err(PeerVerifyError::CredentialsFailed(
                std::io::Error::last_os_error().to_string(),
            ));
        }
        pid
    };

    // Get executable path via proc_pidpath
    let exe_path = {
        const PROC_PIDPATHINFO_MAXSIZE: usize = 4096;
        let mut buf = vec![0u8; PROC_PIDPATHINFO_MAXSIZE];

        extern "C" {
            fn proc_pidpath(
                pid: libc::c_int,
                buffer: *mut libc::c_void,
                buffersize: u32,
            ) -> libc::c_int;
        }

        let len = unsafe { proc_pidpath(pid, buf.as_mut_ptr() as *mut _, buf.len() as u32) };
        if len <= 0 {
            return Err(PeerVerifyError::ProcessNotFound(pid));
        }
        PathBuf::from(
            std::str::from_utf8(&buf[..len as usize])
                .map_err(|_| PeerVerifyError::ProcessNotFound(pid))?,
        )
    };

    // Verify executable is trusted
    verify_executable(&exe_path)?;

    Ok(PeerInfo {
        pid,
        executable: exe_path,
    })
}

/// Verify connecting peer on Windows using GetNamedPipeClientProcessId.
#[cfg(target_os = "windows")]
pub fn verify_peer(pipe: windows::Win32::Foundation::HANDLE) -> Result<PeerInfo, PeerVerifyError> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Pipes::GetNamedPipeClientProcessId;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    // Get client PID
    let mut pid: u32 = 0;
    unsafe {
        GetNamedPipeClientProcessId(pipe, &mut pid)
            .map_err(|e| PeerVerifyError::CredentialsFailed(e.to_string()))?;
    }

    // Get executable path
    let exe_path = unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|_| PeerVerifyError::ProcessNotFound(pid as i32))?;

        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;

        let result = QueryFullProcessImageNameW(
            process,
            Default::default(),
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        );

        let _ = CloseHandle(process);

        result.map_err(|_| PeerVerifyError::ProcessNotFound(pid as i32))?;

        PathBuf::from(String::from_utf16_lossy(&buf[..len as usize]))
    };

    // Verify executable is trusted
    verify_executable(&exe_path)?;

    Ok(PeerInfo {
        pid: pid as i32,
        executable: exe_path,
    })
}
