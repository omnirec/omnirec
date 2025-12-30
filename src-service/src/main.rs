//! OmniRec Background Service
//!
//! This is the background service that handles all capture, encoding, and
//! recording operations. It communicates with the UI client via IPC.

mod capture;
mod encoder;
mod ipc;
mod state;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

/// Set Per-Monitor DPI Awareness v2 for consistent coordinate handling.
///
/// This must be called before any other Windows API calls. It ensures that
/// this process receives physical pixel coordinates from APIs like
/// `GetMonitorInfoW` and `EnumDisplayMonitors`, matching what Tauri reports.
#[cfg(windows)]
fn set_dpi_awareness() {
    use windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };

    unsafe {
        // Per-Monitor DPI Aware v2 gives us:
        // - Physical pixel coordinates from monitor enumeration
        // - Automatic scaling of non-client areas
        // - Correct behavior on multi-monitor setups with different DPIs
        let result = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        if result.is_err() {
            // This can fail if already set (e.g., by a manifest or another call)
            // It's not fatal - we just need to be aware of the current mode
            eprintln!(
                "Warning: SetProcessDpiAwarenessContext failed (may already be set): {:?}",
                result
            );
        }
    }
}

/// Global shutdown flag
static SHUTDOWN_FLAG: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();

/// Get the global shutdown flag.
pub fn get_shutdown_flag() -> Arc<AtomicBool> {
    SHUTDOWN_FLAG
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

/// Request service shutdown.
pub fn request_shutdown() {
    info!("Shutdown requested");
    get_shutdown_flag().store(true, Ordering::SeqCst);
}

/// Check if shutdown has been requested.
pub fn is_shutdown_requested() -> bool {
    get_shutdown_flag().load(Ordering::SeqCst)
}

fn main() {
    // Set DPI awareness before any other Windows API calls
    #[cfg(windows)]
    {
        set_dpi_awareness();
    }

    // Initialize logging with RUST_LOG env var support
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("OmniRec Service starting (pid: {})...", std::process::id());

    // Set up signal handlers for graceful shutdown
    setup_signal_handlers();

    // Initialize FFmpeg (download if needed)
    info!("Initializing FFmpeg...");
    match encoder::ensure_ffmpeg_blocking() {
        Ok(()) => info!("FFmpeg initialized successfully"),
        Err(e) => {
            // FFmpeg is required for encoding, but we can still start the service
            // It will fail when trying to record if FFmpeg is not available
            warn!("FFmpeg initialization failed: {}", e);
        }
    }

    // Run async runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    runtime.block_on(async {
        // Initialize platform-specific capture backends
        #[cfg(target_os = "linux")]
        {
            info!("Initializing Linux capture backends...");
            if let Err(e) = capture::linux::init_ipc_server().await {
                error!("Failed to initialize IPC server: {}", e);
            }
            capture::linux::init_screencopy();
            if let Err(e) = capture::linux::init_audio() {
                error!("Failed to initialize audio: {}", e);
            }
        }

        // Start the IPC server (runs until shutdown)
        if let Err(e) = ipc::run_server().await {
            if !is_shutdown_requested() {
                error!("IPC server error: {}", e);
                std::process::exit(1);
            }
        }
    });

    // Cleanup
    cleanup_on_shutdown();
    info!("OmniRec Service stopped");
}

/// Set up signal handlers for graceful shutdown.
fn setup_signal_handlers() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        // SIGTERM handler
        std::thread::spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                let mut sigterm = signal(SignalKind::terminate()).unwrap();
                let mut sigint = signal(SignalKind::interrupt()).unwrap();
                let mut sighup = signal(SignalKind::hangup()).unwrap();

                tokio::select! {
                    _ = sigterm.recv() => {
                        info!("Received SIGTERM");
                    }
                    _ = sigint.recv() => {
                        info!("Received SIGINT");
                    }
                    _ = sighup.recv() => {
                        info!("Received SIGHUP");
                    }
                }

                request_shutdown();
            });
        });
    }

    #[cfg(windows)]
    {
        // Windows uses Ctrl+C handler
        ctrlc::set_handler(|| {
            info!("Received Ctrl+C");
            request_shutdown();
        })
        .expect("Error setting Ctrl+C handler");
    }
}

/// Cleanup resources on shutdown.
fn cleanup_on_shutdown() {
    info!("Cleaning up...");

    // Stop any active recording
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();

    if let Ok(rt) = rt {
        rt.block_on(async {
            let manager = state::get_recording_manager();
            let current_state = manager.get_state().await;

            if current_state == omnirec_common::RecordingState::Recording {
                info!("Stopping active recording before shutdown...");
                match manager.stop_recording().await {
                    Ok(result) => {
                        info!("Recording saved to: {}", result.file_path.display());
                    }
                    Err(e) => {
                        error!("Failed to stop recording: {}", e);
                    }
                }
            }
        });
    }

    // Remove socket file
    #[cfg(unix)]
    {
        let socket_path = omnirec_common::ipc::get_socket_path();
        if socket_path.exists() {
            if let Err(e) = std::fs::remove_file(&socket_path) {
                warn!("Failed to remove socket file: {}", e);
            } else {
                info!("Removed socket file: {:?}", socket_path);
            }
        }
    }
}
