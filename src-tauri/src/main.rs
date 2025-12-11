// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Detect if running on Wayland and set required environment variables.
#[cfg(target_os = "linux")]
fn configure_wayland_env() {
    // Check for Wayland session indicators
    let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|v| v == "wayland")
            .unwrap_or(false);

    if is_wayland {
        // WebKitGTK has compositing issues on Wayland that cause rendering problems.
        // Disabling compositing mode fixes black screen and other visual glitches.
        if std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_err() {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
            eprintln!("[main] Wayland detected, set WEBKIT_DISABLE_COMPOSITING_MODE=1");
        }
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    configure_wayland_env();

    omnirec_lib::run()
}
