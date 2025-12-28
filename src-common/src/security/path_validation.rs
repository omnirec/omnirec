//! Path validation for output directories and file paths.

use std::path::{Path, PathBuf};

/// Path validation error types.
#[derive(Debug, Clone)]
pub enum PathError {
    /// Path contains directory traversal sequences (..)
    ContainsTraversal,
    /// Path contains null bytes
    ContainsNullByte,
    /// Path is not absolute
    NotAbsolute,
    /// Path is outside allowed directories
    OutsideAllowedDirectory,
    /// Path is too long
    TooLong(usize),
    /// Path does not exist or cannot be canonicalized
    CannotCanonicalize(String),
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathError::ContainsTraversal => write!(f, "Path contains directory traversal"),
            PathError::ContainsNullByte => write!(f, "Path contains null byte"),
            PathError::NotAbsolute => write!(f, "Path must be absolute"),
            PathError::OutsideAllowedDirectory => write!(f, "Path is outside allowed directories"),
            PathError::TooLong(len) => write!(f, "Path too long: {} chars", len),
            PathError::CannotCanonicalize(e) => write!(f, "Cannot canonicalize path: {}", e),
        }
    }
}

impl std::error::Error for PathError {}

/// Maximum path length in characters.
pub const MAX_PATH_LENGTH: usize = 4096;

/// Validate and canonicalize an output directory path.
///
/// This function performs several security checks:
/// 1. Rejects paths containing null bytes
/// 2. Rejects paths that are too long
/// 3. Rejects paths containing ".." traversal sequences
/// 4. Requires absolute paths
/// 5. Canonicalizes the path to resolve symlinks
/// 6. Verifies the path is within allowed directories
pub fn validate_output_directory(path: &Path) -> Result<PathBuf, PathError> {
    let path_str = path.to_string_lossy();

    // Check for null bytes
    if path_str.contains('\0') {
        return Err(PathError::ContainsNullByte);
    }

    // Check length
    if path_str.len() > MAX_PATH_LENGTH {
        return Err(PathError::TooLong(path_str.len()));
    }

    // Reject traversal sequences before canonicalization
    for component in path.components() {
        if component.as_os_str() == ".." {
            return Err(PathError::ContainsTraversal);
        }
    }

    // Must be absolute
    if !path.is_absolute() {
        return Err(PathError::NotAbsolute);
    }

    // Canonicalize to resolve symlinks
    let canonical = std::fs::canonicalize(path)
        .map_err(|e| PathError::CannotCanonicalize(e.to_string()))?;

    // Verify within allowed directories
    let allowed = get_allowed_output_dirs();
    let in_allowed = allowed.iter().any(|base| {
        std::fs::canonicalize(base)
            .map(|b| canonical.starts_with(&b))
            .unwrap_or(false)
    });

    if !in_allowed {
        return Err(PathError::OutsideAllowedDirectory);
    }

    Ok(canonical)
}

/// Get the list of allowed output directories.
///
/// This includes common user-writable locations where recordings
/// can be saved.
fn get_allowed_output_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // User's home directory and common subdirectories
    if let Some(home) = directories::UserDirs::new().map(|u| u.home_dir().to_path_buf()) {
        dirs.push(home.clone());
        dirs.push(home.join("Videos"));
        dirs.push(home.join("Movies"));
        dirs.push(home.join("Documents"));
        dirs.push(home.join("Desktop"));
        dirs.push(home.join("Downloads"));
    }

    // Platform video directory
    if let Some(video) = directories::UserDirs::new().and_then(|u| u.video_dir().map(|p| p.to_path_buf())) {
        dirs.push(video);
    }

    // Temp directory
    dirs.push(std::env::temp_dir());

    // Linux-specific
    #[cfg(target_os = "linux")]
    {
        dirs.push(PathBuf::from("/tmp"));
    }

    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rejects_traversal() {
        let path = PathBuf::from("/home/user/../etc/passwd");
        assert!(matches!(
            validate_output_directory(&path),
            Err(PathError::ContainsTraversal)
        ));
    }

    #[test]
    fn test_rejects_relative_path() {
        let path = PathBuf::from("relative/path");
        assert!(matches!(
            validate_output_directory(&path),
            Err(PathError::NotAbsolute)
        ));
    }

    #[test]
    fn test_rejects_null_byte() {
        let path = PathBuf::from("/home/user\0/Videos");
        assert!(matches!(
            validate_output_directory(&path),
            Err(PathError::ContainsNullByte)
        ));
    }
}
