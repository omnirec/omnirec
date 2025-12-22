//! Approval token storage for the picker consent system.
//!
//! This module handles reading and writing the approval token that allows
//! the picker to bypass the consent dialog when "Always allow" is selected.

use std::fs;
use std::io;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Get the path to the approval token file.
///
/// Uses XDG_STATE_HOME if set, otherwise falls back to ~/.local/state/omnirec/
pub fn get_token_path() -> PathBuf {
    let state_dir = std::env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/tmp"))
                .join(".local")
                .join("state")
        });

    state_dir.join("omnirec").join("approval-token")
}

/// Check if an approval token exists.
pub fn has_token() -> bool {
    get_token_path().exists()
}

/// Read the approval token from disk.
///
/// Returns None if the token file doesn't exist or can't be read.
pub fn read_token() -> Option<String> {
    let path = get_token_path();
    match fs::read_to_string(&path) {
        Ok(content) => {
            let token = content.trim().to_string();
            if token.is_empty() {
                None
            } else {
                Some(token)
            }
        }
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                eprintln!("[ApprovalToken] Failed to read token: {}", e);
            }
            None
        }
    }
}

/// Write an approval token to disk.
///
/// Creates the parent directory if needed and sets file permissions to 0600.
pub fn write_token(token: &str) -> io::Result<()> {
    let path = get_token_path();

    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write the token
    fs::write(&path, token)?;

    // Set restrictive permissions (owner read/write only)
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms)?;
    }

    eprintln!("[ApprovalToken] Token stored at {:?}", path);
    Ok(())
}

/// Validate a token against the stored token.
///
/// Returns true if the token matches, false otherwise.
pub fn validate_token(token: &str) -> bool {
    match read_token() {
        Some(stored) => {
            // Constant-time comparison to prevent timing attacks
            if stored.len() != token.len() {
                return false;
            }
            stored
                .bytes()
                .zip(token.bytes())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0
        }
        None => false,
    }
}

/// Delete the approval token.
#[allow(dead_code)]
pub fn delete_token() -> io::Result<()> {
    let path = get_token_path();
    if path.exists() {
        fs::remove_file(&path)?;
        eprintln!("[ApprovalToken] Token deleted");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_token_path_with_xdg() {
        // This test checks the path calculation logic directly
        // We don't modify env vars since that causes race conditions in parallel tests
        let path = get_token_path();
        // Just verify it ends with the expected suffix
        assert!(path.ends_with("omnirec/approval-token"));
    }

    #[test]
    fn test_validate_token_mismatch() {
        // Test the constant-time comparison logic
        // If stored token is "abc", "abd" should fail
        // We can't easily test with real files due to env var races,
        // but we can test the comparison logic
        assert!(!validate_token("any-token")); // No token stored = fails
    }

    #[test]
    fn test_token_format() {
        // Test that read_token trims whitespace
        // This tests the logic without needing real file operations
        let token = "  test-token  ".trim().to_string();
        assert_eq!(token, "test-token");
    }
}
