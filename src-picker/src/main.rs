//! OmniRec Picker - Custom picker for xdg-desktop-portal-hyprland.
//!
//! This is a headless picker that xdg-desktop-portal-hyprland invokes instead
//! of the standard hyprland-share-picker. Instead of showing a UI, it queries
//! the main omnirec app via IPC for the user's pre-selected capture
//! target and outputs it to stdout in the format XDPH expects.
//!
//! # How it works
//!
//! 1. XDPH invokes this binary when a screencast request needs source selection
//! 2. We connect to the main app's IPC socket
//! 3. We query for the current capture selection
//! 4. We output the selection to stdout in XDPH format: `[SELECTION]/<type>:<id>`
//! 5. XDPH parses our output and continues the portal flow
//!
//! # Output format (same as hyprland-share-picker)
//!
//! - Screen: `[SELECTION]/screen:<output_name>`
//! - Window: `[SELECTION]/window:<window_handle>`
//! - Region: `[SELECTION]/region:<output>@<x>,<y>,<w>,<h>`
//!
//! # Fallback behavior
//!
//! If OmniRec is not running or has no active selection, this picker falls back
//! to the standard `hyprland-share-picker` so that other applications (OBS, Zoom,
//! Discord, etc.) can still request screen capture.

mod ipc_client;

use ipc_client::{query_selection, IpcResponse};
use std::io::{BufRead, BufReader};
use std::process::{Command, ExitCode, Stdio};

/// Default fallback picker binary name.
/// Can be overridden via OMNIREC_FALLBACK_PICKER environment variable.
const DEFAULT_FALLBACK_PICKER: &str = "hyprland-share-picker";

/// Window entry from XDPH's window list.
#[derive(Debug)]
struct WindowEntry {
    /// XDPH's internal handle ID (lower 32 bits)
    handle_id: u64,
    /// Window class
    class: String,
    /// Window title
    title: String,
    /// Hyprland window address
    window_addr: u64,
}

/// Parse the XDPH_WINDOW_SHARING_LIST environment variable.
/// Format: <id>[HC>]<class>[HT>]<title>[HE>]<window_addr>[HA>]...
fn parse_window_list(env_value: &str) -> Vec<WindowEntry> {
    let mut windows = Vec::new();
    let mut remaining = env_value;

    while !remaining.is_empty() {
        // Parse ID
        let Some(id_end) = remaining.find("[HC>]") else {
            break;
        };
        let id_str = &remaining[..id_end];

        // Parse class
        remaining = &remaining[id_end + 5..];
        let Some(class_end) = remaining.find("[HT>]") else {
            break;
        };
        let class = &remaining[..class_end];

        // Parse title
        remaining = &remaining[class_end + 5..];
        let Some(title_end) = remaining.find("[HE>]") else {
            break;
        };
        let title = &remaining[..title_end];

        // Parse window address
        remaining = &remaining[title_end + 5..];
        let Some(addr_end) = remaining.find("[HA>]") else {
            break;
        };
        let addr_str = &remaining[..addr_end];

        // Move past this entry
        remaining = &remaining[addr_end + 5..];

        // Parse the values
        let handle_id = id_str.parse::<u64>().unwrap_or(0);
        let window_addr = addr_str.parse::<u64>().unwrap_or(0);

        windows.push(WindowEntry {
            handle_id,
            class: class.to_string(),
            title: title.to_string(),
            window_addr,
        });
    }

    windows
}

/// Find the XDPH handle ID for a given Hyprland window address.
fn find_window_handle(windows: &[WindowEntry], hyprland_addr: u64) -> Option<u64> {
    windows
        .iter()
        .find(|w| w.window_addr == hyprland_addr)
        .map(|w| w.handle_id)
}

/// Run the fallback picker (standard hyprland-share-picker) and forward its output.
///
/// This is called when OmniRec is not running or has no active selection,
/// allowing other applications to still request screen capture.
fn run_fallback_picker() -> ExitCode {
    let picker_binary = std::env::var("OMNIREC_FALLBACK_PICKER")
        .unwrap_or_else(|_| DEFAULT_FALLBACK_PICKER.to_string());

    eprintln!(
        "[omnirec-picker] Falling back to standard picker: {}",
        picker_binary
    );

    // Execute the standard picker, inheriting all environment variables
    let mut child = match Command::new(&picker_binary)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            eprintln!(
                "[omnirec-picker] Failed to execute fallback picker '{}': {}",
                picker_binary, e
            );
            if e.kind() == std::io::ErrorKind::NotFound {
                eprintln!(
                    "[omnirec-picker] Make sure '{}' is installed and in PATH",
                    picker_binary
                );
            }
            return ExitCode::FAILURE;
        }
    };

    // Forward stdout from the fallback picker to our stdout
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    eprintln!("[omnirec-picker] Fallback picker output: {}", line);
                    println!("{}", line);
                }
                Err(e) => {
                    eprintln!("[omnirec-picker] Error reading fallback picker output: {}", e);
                }
            }
        }
    }

    // Wait for the fallback picker to exit and return its exit code
    match child.wait() {
        Ok(status) => {
            let code = status.code().unwrap_or(1);
            eprintln!("[omnirec-picker] Fallback picker exited with code: {}", code);
            if code == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("[omnirec-picker] Failed to wait for fallback picker: {}", e);
            ExitCode::FAILURE
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    // Log that we were invoked (visible in journalctl)
    eprintln!("[omnirec-picker] Picker invoked");

    // Query the main app for the current selection
    let response = match query_selection().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[omnirec-picker] Failed to query main app: {}", e);
            eprintln!("[omnirec-picker] OmniRec may not be running, trying fallback picker");
            return run_fallback_picker();
        }
    };

    match response {
        IpcResponse::Selection {
            source_type,
            source_id,
            geometry,
        } => {
            eprintln!(
                "[omnirec-picker] Got selection: type={}, id={}",
                source_type, source_id
            );

            // Format output for XDPH
            let output = match source_type.as_str() {
                "monitor" => {
                    format!("[SELECTION]/screen:{}", source_id)
                }
                "window" => {
                    // Parse our source_id (Hyprland window address like "0x55df589f63d0")
                    let hyprland_addr = if let Some(stripped) = source_id.strip_prefix("0x") {
                        u64::from_str_radix(stripped, 16).unwrap_or(0)
                    } else {
                        source_id.parse::<u64>().unwrap_or(0)
                    };

                    eprintln!(
                        "[omnirec-picker] Looking for window with Hyprland addr: 0x{:x}",
                        hyprland_addr
                    );

                    // Get the window list from XDPH
                    let window_list = std::env::var("XDPH_WINDOW_SHARING_LIST").unwrap_or_default();
                    let windows = parse_window_list(&window_list);

                    eprintln!(
                        "[omnirec-picker] XDPH provided {} windows",
                        windows.len()
                    );
                    for w in &windows {
                        eprintln!(
                            "[omnirec-picker]   handle={}, addr=0x{:x}, class={}, title={}",
                            w.handle_id, w.window_addr, w.class, w.title
                        );
                    }

                    // Find the XDPH handle for our window
                    match find_window_handle(&windows, hyprland_addr) {
                        Some(handle) => {
                            eprintln!(
                                "[omnirec-picker] Found XDPH handle: {}",
                                handle
                            );
                            format!("[SELECTION]/window:{}", handle)
                        }
                        None => {
                            eprintln!(
                                "[omnirec-picker] Window not found in XDPH list, trying direct address"
                            );
                            // Fallback: try using the address directly (may not work)
                            format!("[SELECTION]/window:{}", hyprland_addr)
                        }
                    }
                }
                "region" => {
                    // Region format: screen@x,y,w,h
                    if let Some(geom) = geometry {
                        eprintln!("[omnirec-picker] Region selection: {}@{},{},{},{}", 
                            source_id, geom.x, geom.y, geom.width, geom.height);
                        format!(
                            "[SELECTION]/region:{}@{},{},{},{}",
                            source_id, geom.x, geom.y, geom.width, geom.height
                        )
                    } else {
                        eprintln!("[omnirec-picker] Region selection missing geometry");
                        return ExitCode::FAILURE;
                    }
                }
                _ => {
                    eprintln!(
                        "[omnirec-picker] Unknown source type: {}",
                        source_type
                    );
                    return ExitCode::FAILURE;
                }
            };

            eprintln!("[omnirec-picker] Output: {}", output);

            // Output to stdout - this is what XDPH reads
            println!("{}", output);
            ExitCode::SUCCESS
        }
        IpcResponse::NoSelection => {
            eprintln!("[omnirec-picker] No capture selection available in main app");
            eprintln!("[omnirec-picker] Trying fallback picker for user selection");
            return run_fallback_picker();
        }
        IpcResponse::Error { message } => {
            eprintln!("[omnirec-picker] Error from main app: {}", message);
            ExitCode::FAILURE
        }
    }
}
