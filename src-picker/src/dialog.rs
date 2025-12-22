//! Approval dialog for screen recording consent.
//!
//! This module provides a Qt6 dialog that asks the user to approve
//! OmniRec's screen recording request.

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Result of the approval dialog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DialogResult {
    /// User approved the recording request.
    Approved {
        /// Whether "Always allow" was checked.
        always_allow: bool,
    },
    /// User denied the recording request.
    Denied,
}

/// Get the path to the omnirec-dialog binary.
/// 
/// Searches in order:
/// 1. Next to the picker binary (for development)
/// 2. In the same directory as the picker (installed)
/// 3. In PATH
fn find_dialog_binary() -> Option<PathBuf> {
    // Check next to this binary (dev build location)
    if let Ok(exe) = std::env::current_exe() {
        // Development: qt-dialog/build/omnirec-dialog relative to picker binary
        if let Some(picker_dir) = exe.parent() {
            // picker is at src-picker/target/release/omnirec-picker
            // dialog is at src-picker/qt-dialog/build/omnirec-dialog
            let dev_path = picker_dir
                .parent() // target
                .and_then(|p| p.parent()) // src-picker
                .map(|p| p.join("qt-dialog").join("build").join("omnirec-dialog"));
            
            if let Some(path) = dev_path {
                if path.exists() {
                    return Some(path);
                }
            }
            
            // Installed: same directory as picker
            let installed_path = picker_dir.join("omnirec-dialog");
            if installed_path.exists() {
                return Some(installed_path);
            }
        }
    }
    
    // Check PATH
    if let Ok(output) = Command::new("which").arg("omnirec-dialog").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }
    
    None
}

/// Show the approval dialog and wait for user response.
///
/// Uses our custom Qt6 dialog (omnirec-dialog) for a polished UI.
/// Falls back to hyprland-dialog if omnirec-dialog is not available.
///
/// # Arguments
/// * `source_type` - Type of capture source (e.g., "monitor", "window", "region")
/// * `source_id` - Identifier of the capture source (e.g., "DP-1", window title)
pub fn show_approval_dialog(source_type: &str, source_id: &str) -> DialogResult {
    eprintln!("[omnirec-picker] show_approval_dialog called");

    let source_desc = match source_type {
        "monitor" => format!("Display: {}", source_id),
        "window" => format!("Window: {}", source_id),
        "region" => format!("Region on: {}", source_id),
        _ => format!("Source: {}", source_id),
    };

    // Try our custom Qt dialog first
    if let Some(result) = try_omnirec_dialog(&source_desc) {
        return result;
    }

    // Fall back to hyprland-dialog
    if let Some(result) = try_hyprland_dialog(&source_desc) {
        return result;
    }

    // No dialog tool available - deny by default for security
    eprintln!("[omnirec-picker] ERROR: No dialog tool available!");
    eprintln!("[omnirec-picker] Please ensure omnirec-dialog is built or hyprland-guiutils is installed");
    DialogResult::Denied
}

/// Try to show dialog using our custom omnirec-dialog Qt binary.
fn try_omnirec_dialog(source_desc: &str) -> Option<DialogResult> {
    let dialog_path = find_dialog_binary()?;
    
    eprintln!("[omnirec-picker] Using omnirec-dialog at {:?}", dialog_path);

    let result = Command::new(&dialog_path)
        .arg(source_desc)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let response = stdout.trim();
            eprintln!(
                "[omnirec-picker] omnirec-dialog response: '{}' (exit: {:?})",
                response,
                output.status.code()
            );

            match response {
                "ALWAYS_ALLOW" => {
                    eprintln!("[omnirec-picker] User approved with always_allow=true");
                    Some(DialogResult::Approved { always_allow: true })
                }
                "ALLOW_ONCE" => {
                    eprintln!("[omnirec-picker] User approved with always_allow=false");
                    Some(DialogResult::Approved { always_allow: false })
                }
                _ => {
                    eprintln!("[omnirec-picker] User denied");
                    Some(DialogResult::Denied)
                }
            }
        }
        Err(e) => {
            eprintln!("[omnirec-picker] omnirec-dialog failed: {}", e);
            None
        }
    }
}

/// Try to show dialog using hyprland-dialog (fallback).
fn try_hyprland_dialog(source_desc: &str) -> Option<DialogResult> {
    eprintln!("[omnirec-picker] Trying hyprland-dialog (fallback)");

    let text = format!(
        "OmniRec is requesting permission to record your screen.\n\n{}",
        source_desc
    );

    let result = Command::new("hyprland-dialog")
        .args([
            "--title",
            "OmniRec - Screen Recording Permission",
            "--text",
            &text,
            "--buttons",
            "Always Allow;Allow Once;Deny",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let response = stdout.trim();
            eprintln!(
                "[omnirec-picker] hyprland-dialog response: '{}' (exit: {:?})",
                response,
                output.status.code()
            );

            match response {
                "Always Allow" => {
                    eprintln!("[omnirec-picker] User approved with always_allow=true");
                    Some(DialogResult::Approved { always_allow: true })
                }
                "Allow Once" => {
                    eprintln!("[omnirec-picker] User approved with always_allow=false");
                    Some(DialogResult::Approved { always_allow: false })
                }
                _ => {
                    eprintln!("[omnirec-picker] User denied");
                    Some(DialogResult::Denied)
                }
            }
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                eprintln!("[omnirec-picker] hyprland-dialog not found");
                None
            } else {
                eprintln!("[omnirec-picker] hyprland-dialog failed: {}", e);
                Some(DialogResult::Denied)
            }
        }
    }
}

/// Generate a random 256-bit approval token as a hex string.
pub fn generate_approval_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token_length() {
        let token = generate_approval_token();
        assert_eq!(token.len(), 64); // 32 bytes * 2 hex chars
    }

    #[test]
    fn test_generate_token_uniqueness() {
        let token1 = generate_approval_token();
        let token2 = generate_approval_token();
        assert_ne!(token1, token2);
    }

    #[test]
    fn test_generate_token_is_hex() {
        let token = generate_approval_token();
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
