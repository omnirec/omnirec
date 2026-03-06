//! Platform-specific logging directory resolution.

use std::path::PathBuf;

/// Returns the platform-appropriate directory for log files.
///
/// | Platform | Directory |
/// |----------|-----------|
/// | Linux | `$XDG_STATE_HOME/omnirec/logs` or `~/.local/state/omnirec/logs` |
/// | macOS | `~/Library/Logs/omnirec` |
/// | Windows | `%LOCALAPPDATA%\omnirec\omnirec\logs` |
pub fn log_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let base = directories::ProjectDirs::from("", "", "omnirec")
            .expect("Failed to determine project directories");
        base.state_dir()
            .unwrap_or_else(|| base.data_local_dir().join("state"))
            .join("logs")
    }

    #[cfg(target_os = "macos")]
    {
        // ~/Library/Logs/<app>/ is the standard macOS log location.
        // `directories` doesn't expose this path directly, so we derive it
        // from the data_local_dir (~/Library/Application Support/omnirec) by
        // walking up to ~/Library and appending "Logs/omnirec".
        let base = directories::ProjectDirs::from("", "", "omnirec")
            .expect("Failed to determine project directories");
        // data_local_dir → ~/Library/Application Support/omnirec
        // parent         → ~/Library/Application Support
        // parent         → ~/Library
        let library = base
            .data_local_dir()
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| base.data_local_dir().to_path_buf());
        library.join("Logs").join("omnirec")
    }

    #[cfg(target_os = "windows")]
    {
        let base = directories::ProjectDirs::from("", "", "omnirec")
            .expect("Failed to determine project directories");
        base.data_local_dir().join("logs")
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let base = directories::ProjectDirs::from("", "", "omnirec")
            .expect("Failed to determine project directories");
        base.data_local_dir().join("logs")
    }
}

/// Ensures the log directory exists, creating it if necessary.
///
/// Returns `Ok(())` if the directory exists or was created.
/// Returns `Err` if the directory could not be created.
pub fn ensure_log_dir() -> Result<(), std::io::Error> {
    let dir = log_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(())
}

/// Returns the base path for the application log file.
///
/// The rolling appender will use this path's parent directory and create
/// dated files (e.g. `omnirec-app.2026-03-01.log`).
pub fn app_log_path() -> PathBuf {
    log_dir().join("omnirec-app.log")
}
