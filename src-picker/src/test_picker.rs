//! Test CLI for omnirec-picker passthrough functionality.
//!
//! This tool triggers a portal screencast request to test the picker behavior
//! and provides diagnostic information about the environment.
//!
//! Usage:
//!   omnirec-picker-test [--monitor | --window]
//!
//! Options:
//!   --monitor  Request monitor/screen capture (default)
//!   --window   Request window capture

mod ipc_client;

use ashpd::desktop::screencast::{CursorMode, Screencast, SourceType};
use ashpd::desktop::PersistMode;
use ashpd::enumflags2::BitFlags;
use ashpd::WindowIdentifier;
use ipc_client::{query_selection, IpcResponse};
use std::path::PathBuf;

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

fn print_header(text: &str) {
    println!("\n{BOLD}{CYAN}=== {text} ==={RESET}\n");
}

fn print_ok(label: &str, value: &str) {
    println!("  {GREEN}✓{RESET} {label}: {value}");
}

fn print_warn(label: &str, value: &str) {
    println!("  {YELLOW}⚠{RESET} {label}: {value}");
}

fn print_err(label: &str, value: &str) {
    println!("  {RED}✗{RESET} {label}: {value}");
}

fn print_info(label: &str, value: &str) {
    println!("  • {label}: {value}");
}

/// Get the IPC socket path.
fn get_socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(runtime_dir)
        .join("omnirec")
        .join("picker.sock")
}

/// Check environment and print diagnostics.
fn check_environment() {
    print_header("Environment Diagnostics");

    // Hyprland
    match std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        Ok(sig) => print_ok("Hyprland", &format!("Running ({})", sig)),
        Err(_) => print_err("Hyprland", "Not detected (HYPRLAND_INSTANCE_SIGNATURE not set)"),
    }

    // XDG_RUNTIME_DIR
    match std::env::var("XDG_RUNTIME_DIR") {
        Ok(dir) => print_ok("XDG_RUNTIME_DIR", &dir),
        Err(_) => print_err("XDG_RUNTIME_DIR", "Not set"),
    }

    // Wayland display
    match std::env::var("WAYLAND_DISPLAY") {
        Ok(display) => print_ok("Wayland Display", &display),
        Err(_) => print_warn("Wayland Display", "Not set"),
    }

    // Check for standard picker
    let standard_picker = which_picker("hyprland-share-picker");
    match &standard_picker {
        Some(path) => print_ok("Standard Picker", path),
        None => print_warn("Standard Picker", "hyprland-share-picker not found in PATH"),
    }

    // Check for omnirec-picker
    let omnirec_picker = which_picker("omnirec-picker");
    match &omnirec_picker {
        Some(path) => print_ok("OmniRec Picker", path),
        None => print_warn("OmniRec Picker", "omnirec-picker not found in PATH"),
    }

    // Check fallback picker env var
    match std::env::var("OMNIREC_FALLBACK_PICKER") {
        Ok(picker) => print_info("Fallback Override", &picker),
        Err(_) => print_info("Fallback Override", "(not set, will use hyprland-share-picker)"),
    }

    // IPC socket
    let socket_path = get_socket_path();
    if socket_path.exists() {
        print_ok("IPC Socket", &format!("{} (exists)", socket_path.display()));
    } else {
        print_warn("IPC Socket", &format!("{} (not found - OmniRec may not be running)", socket_path.display()));
    }
}

/// Find a binary in PATH.
fn which_picker(name: &str) -> Option<String> {
    std::env::var("PATH")
        .ok()?
        .split(':')
        .map(|p| PathBuf::from(p).join(name))
        .find(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
}

/// Test IPC connection to OmniRec.
async fn test_ipc_connection() {
    print_header("IPC Connection Test");

    match query_selection().await {
        Ok(response) => {
            print_ok("Connection", "Successfully connected to OmniRec");
            match response {
                IpcResponse::Selection { source_type, source_id, geometry, has_approval_token } => {
                    print_ok("Selection", &format!("type={}, id={}", source_type, source_id));
                    if let Some(geom) = geometry {
                        print_info("Geometry", &format!("{}x{} at ({}, {})", geom.width, geom.height, geom.x, geom.y));
                    }
                    if has_approval_token {
                        print_ok("Approval Token", "Present (will auto-approve)");
                    } else {
                        print_warn("Approval Token", "Not present (will show approval dialog)");
                    }
                    println!("\n  {CYAN}→ Picker will use OmniRec's selection{RESET}");
                }
                IpcResponse::NoSelection => {
                    print_warn("Selection", "No active selection in OmniRec");
                    println!("\n  {CYAN}→ Picker will fall back to standard picker{RESET}");
                }
                IpcResponse::Error { message } => {
                    print_err("Response", &format!("Error from OmniRec: {}", message));
                }
                IpcResponse::TokenValid | IpcResponse::TokenInvalid | IpcResponse::TokenStored => {
                    print_err("Response", "Unexpected response type from QuerySelection");
                }
            }
        }
        Err(e) => {
            print_err("Connection", &format!("Failed: {}", e));
            println!("\n  {CYAN}→ Picker will fall back to standard picker{RESET}");
        }
    }
}

/// Trigger a portal screencast request.
async fn trigger_portal_request(source_type: SourceType) -> Result<(), String> {
    let type_name = match source_type {
        SourceType::Monitor => "Monitor",
        SourceType::Window => "Window",
        _ => "Unknown",
    };
    
    print_header(&format!("Portal Request ({type_name})"));

    println!("  Initiating portal screencast request...");
    println!("  {YELLOW}(This will invoke the configured picker){RESET}\n");

    // Connect to screencast portal
    let screencast = Screencast::new()
        .await
        .map_err(|e| format!("Failed to connect to screencast portal: {}", e))?;
    
    print_ok("Portal", "Connected to org.freedesktop.portal.ScreenCast");

    // Create session
    let session = screencast
        .create_session()
        .await
        .map_err(|e| format!("Failed to create session: {}", e))?;
    
    print_ok("Session", "Created");

    // Select sources (this triggers the picker)
    println!("\n  {BOLD}Triggering picker...{RESET}");
    println!("  {YELLOW}If OmniRec is running with a selection, it will auto-approve.{RESET}");
    println!("  {YELLOW}Otherwise, the standard picker dialog should appear.{RESET}\n");

    let source_types: BitFlags<SourceType> = source_type.into();
    
    screencast
        .select_sources(
            &session,
            CursorMode::Embedded,
            source_types,
            false, // multiple sources
            None,  // restore token
            PersistMode::DoNot,
        )
        .await
        .map_err(|e| format!("Failed to select sources: {}", e))?;
    
    print_ok("Select Sources", "Completed");

    // Start the screencast
    let response = screencast
        .start(&session, &WindowIdentifier::default())
        .await
        .map_err(|e| format!("Failed to start screencast: {}", e))?;

    // Get the result
    let streams = response
        .response()
        .map_err(|e| format!("Portal request rejected: {}", e))?;

    let all_streams = streams.streams();
    
    if all_streams.is_empty() {
        print_warn("Streams", "No streams returned (user may have cancelled)");
        return Ok(());
    }

    print_ok("Streams", &format!("{} stream(s) returned", all_streams.len()));

    for (i, stream) in all_streams.iter().enumerate() {
        println!("\n  {BOLD}Stream {}:{RESET}", i + 1);
        print_info("PipeWire Node ID", &stream.pipe_wire_node_id().to_string());
        if let Some(source_type) = stream.source_type() {
            print_info("Source Type", &format!("{:?}", source_type));
        }
        if let Some((w, h)) = stream.size() {
            print_info("Size", &format!("{}x{}", w, h));
        }
    }

    Ok(())
}

fn print_usage() {
    println!("{BOLD}omnirec-picker-test{RESET} - Test picker passthrough functionality\n");
    println!("Usage: omnirec-picker-test [OPTIONS]\n");
    println!("Options:");
    println!("  --monitor    Request monitor/screen capture (default)");
    println!("  --window     Request window capture");
    println!("  --help       Show this help message\n");
    println!("This tool will:");
    println!("  1. Check environment and dependencies");
    println!("  2. Test IPC connection to OmniRec");
    println!("  3. Trigger a portal screencast request");
    println!("  4. Report the result\n");
    println!("The picker behavior depends on OmniRec state:");
    println!("  • OmniRec running with selection → auto-approve (no dialog)");
    println!("  • OmniRec running, no selection  → standard picker dialog");
    println!("  • OmniRec not running            → standard picker dialog");
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    // Parse arguments
    let mut source_type = SourceType::Monitor;
    
    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--monitor" => source_type = SourceType::Monitor,
            "--window" => source_type = SourceType::Window,
            "--help" | "-h" => {
                print_usage();
                return;
            }
            other => {
                eprintln!("{RED}Unknown option: {other}{RESET}");
                eprintln!("Use --help for usage information.");
                std::process::exit(1);
            }
        }
    }

    println!("{BOLD}{CYAN}");
    println!("╔═══════════════════════════════════════════╗");
    println!("║     OmniRec Picker Passthrough Test       ║");
    println!("╚═══════════════════════════════════════════╝{RESET}");

    // Run diagnostics
    check_environment();
    
    // Test IPC
    test_ipc_connection().await;
    
    // Trigger portal request (async because ashpd requires it)
    match trigger_portal_request(source_type).await {
        Ok(()) => {
            print_header("Result");
            println!("  {GREEN}{BOLD}✓ Portal request completed successfully{RESET}");
        }
        Err(e) => {
            print_header("Result");
            println!("  {RED}{BOLD}✗ Portal request failed: {e}{RESET}");
            std::process::exit(1);
        }
    }
}
