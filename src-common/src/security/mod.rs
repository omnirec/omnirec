//! Security modules for IPC authentication and validation.

pub mod path_validation;
pub mod peer_verify;
pub mod validation;

/// Executable names permitted to connect to the service.
pub const TRUSTED_EXECUTABLES: &[&str] = &["omnirec", "omnirec-service", "omnirec-picker"];

/// Trusted installation directories (Linux).
#[cfg(target_os = "linux")]
pub const TRUSTED_DIRECTORIES: &[&str] = &["/usr/bin", "/usr/local/bin", "/opt/omnirec/bin"];

/// Trusted installation directories (macOS).
#[cfg(target_os = "macos")]
pub const TRUSTED_DIRECTORIES: &[&str] = &[
    "/Applications/OmniRec.app/Contents/MacOS",
    "/usr/local/bin",
    "/opt/homebrew/bin",
];

/// Trusted installation directories (Windows).
#[cfg(target_os = "windows")]
pub const TRUSTED_DIRECTORIES: &[&str] = &[
    r"C:\Program Files\OmniRec",
    r"C:\Program Files (x86)\OmniRec",
];
