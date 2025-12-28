//! IPC client for communicating with the OmniRec service.
//!
//! This module provides a client for sending requests to and receiving
//! responses from the omnirec-service background process.

mod client;

pub use client::{ServiceClient, ServiceError};

/// Get the platform-specific socket path for the service.
pub fn get_socket_path() -> std::path::PathBuf {
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

    #[cfg(target_os = "windows")]
    {
        std::path::PathBuf::from(r"\\.\pipe\omnirec-service")
    }
}
