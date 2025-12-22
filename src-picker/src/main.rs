//! OmniRec Picker - Custom picker for xdg-desktop-portal-hyprland.
//!
//! This picker is invoked by xdg-desktop-portal-hyprland (XDPH) when a screencast
//! request needs source selection. It queries the main OmniRec app for the user's
//! capture selection and outputs it to stdout in XDPH format.
//!
//! # Usage
//!
//! Normal mode (invoked by XDPH):
//!   omnirec-picker
//!
//! Dry-run mode (for testing the dialog):
//!   omnirec-picker --dry-run [--source-type monitor|window|region] [--source-id ID]

mod dialog;
mod ipc_client;

use dialog::{generate_approval_token, show_approval_dialog, DialogResult};
use ipc_client::{query_selection, store_token, IpcResponse};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, ExitCode, Stdio};

/// Log to a file since stderr doesn't go to journal when run by XDPH
fn log(msg: &str) {
    // Also print to stderr for manual testing
    eprintln!("{}", msg);
    
    // Write to a log file
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/omnirec-picker.log")
    {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "[{}] {}", timestamp, msg);
    }
}

/// Default fallback picker binary name.
const DEFAULT_FALLBACK_PICKER: &str = "hyprland-share-picker";

/// Window entry from XDPH's window list.
#[derive(Debug)]
#[allow(dead_code)]
struct WindowEntry {
    handle_id: u64,
    class: String,
    title: String,
    window_addr: u64,
}

/// Parse the XDPH_WINDOW_SHARING_LIST environment variable.
fn parse_window_list(env_value: &str) -> Vec<WindowEntry> {
    let mut windows = Vec::new();
    let mut remaining = env_value;

    while !remaining.is_empty() {
        let Some(id_end) = remaining.find("[HC>]") else { break };
        let id_str = &remaining[..id_end];

        remaining = &remaining[id_end + 5..];
        let Some(class_end) = remaining.find("[HT>]") else { break };
        let class = &remaining[..class_end];

        remaining = &remaining[class_end + 5..];
        let Some(title_end) = remaining.find("[HE>]") else { break };
        let title = &remaining[..title_end];

        remaining = &remaining[title_end + 5..];
        let Some(addr_end) = remaining.find("[HA>]") else { break };
        let addr_str = &remaining[..addr_end];

        remaining = &remaining[addr_end + 5..];

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

fn find_window_handle(windows: &[WindowEntry], hyprland_addr: u64) -> Option<u64> {
    windows
        .iter()
        .find(|w| w.window_addr == hyprland_addr)
        .map(|w| w.handle_id)
}

fn run_fallback_picker() -> ExitCode {
    let picker_binary = std::env::var("OMNIREC_FALLBACK_PICKER")
        .unwrap_or_else(|_| DEFAULT_FALLBACK_PICKER.to_string());

    eprintln!("[omnirec-picker] Falling back to standard picker: {}", picker_binary);

    let mut child = match Command::new(&picker_binary)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            eprintln!("[omnirec-picker] Failed to execute fallback picker '{}': {}", picker_binary, e);
            return ExitCode::FAILURE;
        }
    };

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            eprintln!("[omnirec-picker] Fallback picker output: {}", line);
            println!("{}", line);
        }
    }

    match child.wait() {
        Ok(status) => {
            if status.success() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Err(_) => ExitCode::FAILURE,
    }
}

/// Parse command line arguments for dry-run mode
struct Args {
    dry_run: bool,
    source_type: String,
    source_id: String,
}

fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    let mut result = Args {
        dry_run: false,
        source_type: "monitor".to_string(),
        source_id: "DP-1".to_string(),
    };
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dry-run" => result.dry_run = true,
            "--source-type" => {
                i += 1;
                if i < args.len() {
                    result.source_type = args[i].clone();
                }
            }
            "--source-id" => {
                i += 1;
                if i < args.len() {
                    result.source_id = args[i].clone();
                }
            }
            "--help" | "-h" => {
                eprintln!("Usage: omnirec-picker [OPTIONS]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --dry-run              Test the dialog without IPC");
                eprintln!("  --source-type TYPE     Source type: monitor, window, region (default: monitor)");
                eprintln!("  --source-id ID         Source identifier (default: DP-1)");
                eprintln!("  --help, -h             Show this help");
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }
    
    result
}

/// Run in dry-run mode - just test the dialog
fn run_dry_run(source_type: &str, source_id: &str) -> ExitCode {
    eprintln!("[dry-run] Testing dialog with source_type={}, source_id={}", source_type, source_id);
    
    let result = show_approval_dialog(source_type, source_id);
    
    match result {
        DialogResult::Approved { always_allow } => {
            eprintln!("[dry-run] Result: APPROVED (always_allow={})", always_allow);
            if always_allow {
                let token = generate_approval_token();
                eprintln!("[dry-run] Generated token: {}", token);
                eprintln!("[dry-run] (Token not stored in dry-run mode)");
            }
            ExitCode::SUCCESS
        }
        DialogResult::Denied => {
            eprintln!("[dry-run] Result: DENIED");
            ExitCode::FAILURE
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = parse_args();
    
    // Handle dry-run mode
    if args.dry_run {
        return run_dry_run(&args.source_type, &args.source_id);
    }
    
    log("[omnirec-picker] === Picker started ===");
    log(&format!("[omnirec-picker] PID: {}", std::process::id()));
    
    // Log key environment variables
    if let Ok(v) = std::env::var("XDG_RUNTIME_DIR") {
        log(&format!("[omnirec-picker] XDG_RUNTIME_DIR: {}", v));
    }
    
    log("[omnirec-picker] About to query selection...");

    let response = match query_selection().await {
        Ok(r) => {
            log("[omnirec-picker] Got response from IPC");
            r
        }
        Err(e) => {
            log(&format!("[omnirec-picker] Failed to query main app: {}", e));
            return run_fallback_picker();
        }
    };

    log(&format!("[omnirec-picker] Processing response: {:?}", std::mem::discriminant(&response)));

    match response {
        IpcResponse::Selection {
            source_type,
            source_id,
            geometry,
            has_approval_token,
        } => {
            log(&format!("[omnirec-picker] Got selection: type={}, id={}, has_token={}", 
                source_type, source_id, has_approval_token));

            // Check if we need to show approval dialog
            if !has_approval_token {
                log("[omnirec-picker] No approval token, showing dialog...");
                
                // Run dialog in blocking context (tokio is already handling async)
                let st = source_type.clone();
                let si = source_id.clone();
                let result = tokio::task::spawn_blocking(move || {
                    show_approval_dialog(&st, &si)
                }).await.unwrap_or(DialogResult::Denied);
                
                match result {
                    DialogResult::Approved { always_allow } => {
                        log(&format!("[omnirec-picker] User approved (always_allow={})", always_allow));
                        
                        if always_allow {
                            // Generate and store token
                            let token = generate_approval_token();
                            log("[omnirec-picker] Generated approval token, storing via IPC...");
                            
                            if let Err(e) = store_token(&token).await {
                                log(&format!("[omnirec-picker] Failed to store token: {}", e));
                                // Continue anyway - recording will still work, just won't be persistent
                            } else {
                                log("[omnirec-picker] Token stored successfully");
                            }
                        }
                    }
                    DialogResult::Denied => {
                        log("[omnirec-picker] User denied, exiting");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                log("[omnirec-picker] Has approval token, auto-approving");
            }

            let output = match source_type.as_str() {
                "monitor" => format!("[SELECTION]/screen:{}", source_id),
                "window" => {
                    let hyprland_addr = if let Some(stripped) = source_id.strip_prefix("0x") {
                        u64::from_str_radix(stripped, 16).unwrap_or(0)
                    } else {
                        source_id.parse::<u64>().unwrap_or(0)
                    };

                    let window_list = std::env::var("XDPH_WINDOW_SHARING_LIST").unwrap_or_default();
                    let windows = parse_window_list(&window_list);

                    match find_window_handle(&windows, hyprland_addr) {
                        Some(handle) => format!("[SELECTION]/window:{}", handle),
                        None => format!("[SELECTION]/window:{}", hyprland_addr),
                    }
                }
                "region" => {
                    if let Some(geom) = geometry {
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
                    eprintln!("[omnirec-picker] Unknown source type: {}", source_type);
                    return ExitCode::FAILURE;
                }
            };

            log(&format!("[omnirec-picker] Output: {}", output));
            println!("{}", output);
            log("[omnirec-picker] Exiting with SUCCESS");
            ExitCode::SUCCESS
        }
        IpcResponse::NoSelection => {
            log("[omnirec-picker] No selection, using fallback picker");
            run_fallback_picker()
        }
        IpcResponse::Error { message } => {
            log(&format!("[omnirec-picker] Error: {}", message));
            ExitCode::FAILURE
        }
        _ => {
            log("[omnirec-picker] Unexpected response");
            ExitCode::FAILURE
        }
    }
}
