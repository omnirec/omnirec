//! Screen Recorder Picker - Custom picker for xdg-desktop-portal-hyprland.
//!
//! This is a headless picker that xdg-desktop-portal-hyprland invokes instead
//! of the standard hyprland-share-picker. Instead of showing a UI, it queries
//! the main screen-recorder app via IPC for the user's pre-selected capture
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
//! If no selection is available or the main app isn't running, we exit with
//! an error, causing XDPH to cancel the portal request.

mod ipc_client;

use ipc_client::{query_selection, IpcResponse};
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    // Query the main app for the current selection
    let response = match query_selection().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[screen-recorder-picker] Failed to query main app: {}", e);
            return ExitCode::FAILURE;
        }
    };

    match response {
        IpcResponse::Selection {
            source_type,
            source_id,
            geometry,
        } => {
            // Format output for XDPH
            // Note: We don't include the 'r' flag (allow token) since our app
            // manages its own session state
            let output = match source_type.as_str() {
                "monitor" => {
                    format!("[SELECTION]/screen:{}", source_id)
                }
                "window" => {
                    // XDPH expects the window handle as a number
                    // Our source_id is the Hyprland window address (hex string like "0x...")
                    // We need to parse it and output as decimal
                    let handle = if source_id.starts_with("0x") {
                        u64::from_str_radix(&source_id[2..], 16).unwrap_or(0)
                    } else {
                        source_id.parse::<u64>().unwrap_or(0)
                    };
                    format!("[SELECTION]/window:{}", handle)
                }
                "region" => {
                    // Region format: screen@x,y,w,h
                    if let Some(geom) = geometry {
                        format!(
                            "[SELECTION]/region:{}@{},{},{},{}",
                            source_id, geom.x, geom.y, geom.width, geom.height
                        )
                    } else {
                        eprintln!("[screen-recorder-picker] Region selection missing geometry");
                        return ExitCode::FAILURE;
                    }
                }
                _ => {
                    eprintln!(
                        "[screen-recorder-picker] Unknown source type: {}",
                        source_type
                    );
                    return ExitCode::FAILURE;
                }
            };

            // Output to stdout - this is what XDPH reads
            println!("{}", output);
            ExitCode::SUCCESS
        }
        IpcResponse::NoSelection => {
            eprintln!("[screen-recorder-picker] No capture selection available in main app");
            // Output nothing - XDPH will cancel the request
            ExitCode::FAILURE
        }
        IpcResponse::Error { message } => {
            eprintln!("[screen-recorder-picker] Error from main app: {}", message);
            ExitCode::FAILURE
        }
    }
}
