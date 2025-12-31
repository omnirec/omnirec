//! CLI command implementations.

use crate::client::{ServiceClient, ServiceError};
use crate::colors;
use crate::exit_codes::ExitCode;
use crate::platform;
use crate::RecordTarget;
use omnirec_common::ipc::{Request, Response};
use omnirec_common::{AudioSourceType, OutputFormat, RecordingState};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// List available windows.
pub async fn list_windows(json: bool, quiet: bool) -> ExitCode {
    let client = ServiceClient::new();

    if let Err(e) = client.connect_or_spawn().await {
        if !quiet {
            eprintln!("{}", colors::error(&e.to_string()));
        }
        return e.to_exit_code();
    }

    match client.request(Request::ListWindows).await {
        Ok(Response::Windows { windows }) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&windows).unwrap());
            } else if windows.is_empty() {
                if !quiet {
                    println!("{}", colors::dim("No capturable windows found."));
                }
            } else {
                // Calculate column widths
                let handle_width = windows
                    .iter()
                    .map(|w| w.handle.to_string().len())
                    .max()
                    .unwrap_or(6)
                    .max(6);
                let process_width = windows
                    .iter()
                    .map(|w| w.process_name.len())
                    .max()
                    .unwrap_or(7)
                    .max(7);

                println!(
                    "{}  {}  {}",
                    colors::pad_left("HANDLE", handle_width, colors::header),
                    colors::pad_left("PROCESS", process_width, colors::header),
                    colors::header("TITLE")
                );
                println!(
                    "{}  {}  {}",
                    "-".repeat(handle_width),
                    "-".repeat(process_width),
                    "-".repeat(5)
                );

                for window in windows {
                    let title = if window.title.len() > 60 {
                        format!("{}...", &window.title[..57])
                    } else {
                        window.title.clone()
                    };
                    let handle_str = window.handle.to_string();
                    println!(
                        "{}  {:<process_width$}  {}",
                        colors::pad_left(&handle_str, handle_width, colors::number),
                        window.process_name,
                        title
                    );
                }
            }
            ExitCode::Success
        }
        Ok(other) => {
            if !quiet {
                eprintln!("{}", colors::error(&format!("Unexpected response: {:?}", other)));
            }
            ExitCode::GeneralError
        }
        Err(e) => {
            if !quiet {
                eprintln!("{}", colors::error(&e.to_string()));
            }
            e.to_exit_code()
        }
    }
}

/// List available displays.
pub async fn list_displays(json: bool, quiet: bool) -> ExitCode {
    let client = ServiceClient::new();

    if let Err(e) = client.connect_or_spawn().await {
        if !quiet {
            eprintln!("{}", colors::error(&e.to_string()));
        }
        return e.to_exit_code();
    }

    match client.request(Request::ListMonitors).await {
        Ok(Response::Monitors { monitors }) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&monitors).unwrap());
            } else if monitors.is_empty() {
                if !quiet {
                    println!("{}", colors::dim("No displays found."));
                }
            } else {
                let id_width = monitors
                    .iter()
                    .map(|m| m.id.len())
                    .max()
                    .unwrap_or(2)
                    .max(2);
                let name_width = monitors
                    .iter()
                    .map(|m| m.name.len())
                    .max()
                    .unwrap_or(4)
                    .max(4);

                println!(
                    "{}  {}  {}  {}  {}",
                    colors::pad_left("ID", id_width, colors::header),
                    colors::pad_left("NAME", name_width, colors::header),
                    colors::pad_left("RESOLUTION", 14, colors::header),
                    colors::pad_left("POSITION", 10, colors::header),
                    colors::header("PRIMARY")
                );
                println!(
                    "{}  {}  {}  {}  {}",
                    "-".repeat(id_width),
                    "-".repeat(name_width),
                    "-".repeat(14),
                    "-".repeat(10),
                    "-".repeat(7)
                );

                for monitor in monitors {
                    let resolution = format!("{}x{}", monitor.width, monitor.height);
                    let position = format!("{},{}", monitor.x, monitor.y);
                    let primary = if monitor.is_primary {
                        colors::yes()
                    } else {
                        colors::no()
                    };
                    println!(
                        "{}  {:<name_width$}  {:<14}  {:<10}  {}",
                        colors::pad_left(&monitor.id, id_width, colors::number),
                        monitor.name,
                        resolution,
                        position,
                        primary
                    );
                }
            }
            ExitCode::Success
        }
        Ok(other) => {
            if !quiet {
                eprintln!("{}", colors::error(&format!("Unexpected response: {:?}", other)));
            }
            ExitCode::GeneralError
        }
        Err(e) => {
            if !quiet {
                eprintln!("{}", colors::error(&e.to_string()));
            }
            e.to_exit_code()
        }
    }
}

/// List available audio sources.
pub async fn list_audio(json: bool, quiet: bool) -> ExitCode {
    let client = ServiceClient::new();

    if let Err(e) = client.connect_or_spawn().await {
        if !quiet {
            eprintln!("{}", colors::error(&e.to_string()));
        }
        return e.to_exit_code();
    }

    match client.request(Request::ListAudioSources).await {
        Ok(Response::AudioSources { sources }) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&sources).unwrap());
            } else if sources.is_empty() {
                if !quiet {
                    println!("{}", colors::dim("No audio sources found."));
                }
            } else {
                let id_width = sources
                    .iter()
                    .map(|s| s.id.len())
                    .max()
                    .unwrap_or(2)
                    .max(2);

                // Separate by type
                let outputs: Vec<_> = sources
                    .iter()
                    .filter(|s| s.source_type == AudioSourceType::Output)
                    .collect();
                let inputs: Vec<_> = sources
                    .iter()
                    .filter(|s| s.source_type == AudioSourceType::Input)
                    .collect();

                if !outputs.is_empty() {
                    println!("{}", colors::bold("System Audio (--audio):"));
                    println!("{}  {}", colors::pad_left("ID", id_width, colors::header), colors::header("NAME"));
                    println!("{}  ----", "-".repeat(id_width));
                    for source in outputs {
                        println!("{}  {}", colors::pad_left(&source.id, id_width, colors::number), source.name);
                    }
                    println!();
                }

                if !inputs.is_empty() {
                    println!("{}", colors::bold("Microphones (--microphone):"));
                    println!("{}  {}", colors::pad_left("ID", id_width, colors::header), colors::header("NAME"));
                    println!("{}  ----", "-".repeat(id_width));
                    for source in inputs {
                        println!("{}  {}", colors::pad_left(&source.id, id_width, colors::number), source.name);
                    }
                }
            }
            ExitCode::Success
        }
        Ok(other) => {
            if !quiet {
                eprintln!("{}", colors::error(&format!("Unexpected response: {:?}", other)));
            }
            ExitCode::GeneralError
        }
        Err(e) => {
            if !quiet {
                eprintln!("{}", colors::error(&e.to_string()));
            }
            e.to_exit_code()
        }
    }
}

/// Start a recording.
pub async fn record(target: RecordTarget, json: bool, quiet: bool, verbose: bool) -> ExitCode {
    // Validate format
    let (options, request) = match &target {
        RecordTarget::Window { handle, options } => {
            if platform::is_portal_mode_desktop() {
                if options.strict {
                    if !quiet {
                        eprintln!(
                            "{}",
                            colors::error(&format!(
                                "Window selection not supported on {} (portal-mode desktop).",
                                platform::desktop_name().unwrap_or_else(|| "this desktop".to_string())
                            ))
                        );
                        eprintln!("Use 'omnirec record portal' or remove --strict to fall back to portal.");
                    }
                    return ExitCode::PortalRequired;
                }
                if !quiet {
                    eprintln!(
                        "{}",
                        colors::warning(&format!(
                            "Window selection not supported on {}. Using portal picker.",
                            platform::desktop_name().unwrap_or_else(|| "this desktop".to_string())
                        ))
                    );
                }
                (options, Request::StartPortalCapture)
            } else {
                (options, Request::StartWindowCapture { window_handle: *handle })
            }
        }
        RecordTarget::Display { id, options } => {
            if platform::is_portal_mode_desktop() {
                if options.strict {
                    if !quiet {
                        eprintln!(
                            "{}",
                            colors::error(&format!(
                                "Display selection not supported on {} (portal-mode desktop).",
                                platform::desktop_name().unwrap_or_else(|| "this desktop".to_string())
                            ))
                        );
                        eprintln!("Use 'omnirec record portal' or remove --strict to fall back to portal.");
                    }
                    return ExitCode::PortalRequired;
                }
                if !quiet {
                    eprintln!(
                        "{}",
                        colors::warning(&format!(
                            "Display selection not supported on {}. Using portal picker.",
                            platform::desktop_name().unwrap_or_else(|| "this desktop".to_string())
                        ))
                    );
                }
                (options, Request::StartPortalCapture)
            } else {
                // We need to get display dimensions - fetch from service
                let client = ServiceClient::new();
                if let Err(e) = client.connect_or_spawn().await {
                    if !quiet {
                        eprintln!("{}", colors::error(&e.to_string()));
                    }
                    return e.to_exit_code();
                }

                let (width, height) = match client.request(Request::ListMonitors).await {
                    Ok(Response::Monitors { monitors }) => {
                        match monitors.iter().find(|m| m.id == *id) {
                            Some(monitor) => (monitor.width, monitor.height),
                            None => {
                                if !quiet {
                                    eprintln!("{}", colors::error(&format!("Display '{}' not found.", id)));
                                    eprintln!("Use 'omnirec list displays' to see available displays.");
                                }
                                return ExitCode::InvalidArguments;
                            }
                        }
                    }
                    Ok(_) => {
                        if !quiet {
                            eprintln!("{}", colors::error("Unexpected response when listing displays."));
                        }
                        return ExitCode::GeneralError;
                    }
                    Err(e) => {
                        if !quiet {
                            eprintln!("{}", colors::error(&e.to_string()));
                        }
                        return e.to_exit_code();
                    }
                };

                (
                    options,
                    Request::StartDisplayCapture {
                        monitor_id: id.clone(),
                        width,
                        height,
                    },
                )
            }
        }
        RecordTarget::Region {
            display,
            x,
            y,
            width,
            height,
            options,
        } => {
            if platform::is_portal_mode_desktop() {
                if options.strict {
                    if !quiet {
                        eprintln!(
                            "{}",
                            colors::error(&format!(
                                "Region selection not supported on {} (portal-mode desktop).",
                                platform::desktop_name().unwrap_or_else(|| "this desktop".to_string())
                            ))
                        );
                        eprintln!("Use 'omnirec record portal' or remove --strict to fall back to portal.");
                    }
                    return ExitCode::PortalRequired;
                }
                if !quiet {
                    eprintln!(
                        "{}",
                        colors::warning(&format!(
                            "Region selection not supported on {}. Using portal picker.",
                            platform::desktop_name().unwrap_or_else(|| "this desktop".to_string())
                        ))
                    );
                }
                (options, Request::StartPortalCapture)
            } else {
                (
                    options,
                    Request::StartRegionCapture {
                        monitor_id: display.clone(),
                        x: *x,
                        y: *y,
                        width: *width,
                        height: *height,
                    },
                )
            }
        }
        RecordTarget::Portal { options } => {
            if !platform::is_portal_supported() {
                if !quiet {
                    eprintln!("{}", colors::error("Portal-based recording is not supported on this platform."));
                    eprintln!("Use 'omnirec record window', 'display', or 'region' instead.");
                }
                return ExitCode::GeneralError;
            }
            (options, Request::StartPortalCapture)
        }
    };

    // Validate output format
    if OutputFormat::parse(&options.format).is_none() {
        if !quiet {
            eprintln!(
                "{}",
                colors::error(&format!(
                    "Invalid format '{}'. Valid formats: mp4, webm, mkv, mov, gif, apng, webp",
                    options.format
                ))
            );
        }
        return ExitCode::InvalidArguments;
    }

    let client = ServiceClient::new();

    if let Err(e) = client.connect_or_spawn().await {
        if !quiet {
            eprintln!("{}", colors::error(&e.to_string()));
        }
        return e.to_exit_code();
    }

    // Health check
    if let Err(e) = client.ping().await {
        if !quiet {
            eprintln!("{}", colors::error(&format!("Service health check failed: {}", e)));
        }
        return ExitCode::ServiceConnectionFailed;
    }

    if verbose && !quiet {
        eprintln!("Connected to service.");
    }

    // Configure audio if specified
    if options.audio.is_some() || options.microphone.is_some() {
        let audio_enabled = options.audio.as_deref() != Some("none")
            || options.microphone.as_deref() != Some("none");
        let source_id = options
            .audio
            .as_ref()
            .filter(|s| *s != "none")
            .cloned();
        let microphone_id = options
            .microphone
            .as_ref()
            .filter(|s| *s != "none")
            .cloned();

        if let Err(e) = client
            .request(Request::SetAudioConfig {
                enabled: audio_enabled,
                source_id,
                microphone_id,
                echo_cancellation: true,
            })
            .await
        {
            if !quiet {
                eprintln!("{}", colors::warning(&format!("Failed to configure audio: {}", e)));
            }
        } else if verbose && !quiet {
            eprintln!("{}", colors::info("Audio configured."));
        }
    }

    // Set output format if not default
    if options.format != "mp4" {
        if let Err(e) = client
            .request(Request::SetOutputFormat {
                format: options.format.clone(),
            })
            .await
        {
            if !quiet {
                eprintln!("{}", colors::warning(&format!("Failed to set output format: {}", e)));
            }
        } else if verbose && !quiet {
            eprintln!("{}", colors::info(&format!("Output format set to {}.", options.format)));
        }
    }

    // Start recording
    match client.request(request).await {
        Ok(Response::RecordingStarted) => {
            if !quiet {
                if json {
                    println!(r#"{{"status": "recording_started"}}"#);
                } else {
                    println!("{}", colors::success("Recording started..."));
                }
            }
        }
        Ok(other) => {
            if !quiet {
                eprintln!("{}", colors::error(&format!("Unexpected response: {:?}", other)));
            }
            return ExitCode::RecordingFailedToStart;
        }
        Err(e) => {
            if !quiet {
                eprintln!("{}", colors::error(&format!("Error starting recording: {}", e)));
            }
            return ExitCode::RecordingFailedToStart;
        }
    }

    // Subscribe to events
    if let Err(e) = client.request(Request::SubscribeEvents).await {
        if verbose && !quiet {
            eprintln!("{}", colors::warning(&format!("Failed to subscribe to events: {}", e)));
        }
    }

    // Set up signal handling for graceful shutdown
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let shutdown_flag_clone = shutdown_flag.clone();

    #[cfg(unix)]
    {
        tokio::spawn(async move {
            let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                .expect("Failed to set up SIGINT handler");
            let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Failed to set up SIGTERM handler");

            tokio::select! {
                _ = sigint.recv() => {}
                _ = sigterm.recv() => {}
            }

            shutdown_flag_clone.store(true, Ordering::SeqCst);
        });
    }

    #[cfg(windows)]
    {
        tokio::spawn(async move {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to set up Ctrl+C handler");
            shutdown_flag_clone.store(true, Ordering::SeqCst);
        });
    }

    // Duration timer
    let start_time = std::time::Instant::now();
    let duration_limit = options.duration.map(std::time::Duration::from_secs);

    // Main loop - wait for completion or shutdown
    let mut last_elapsed = 0u64;
    loop {
        // Check for shutdown signal
        if shutdown_flag.load(Ordering::SeqCst) {
            if !quiet && !json {
                eprintln!("\n{}", colors::info("Stopping recording..."));
            }
            break;
        }

        // Check duration limit
        if let Some(limit) = duration_limit {
            if start_time.elapsed() >= limit {
                if !quiet && !json {
                    eprintln!("\n{}", colors::info("Duration limit reached. Stopping recording..."));
                }
                break;
            }
        }

        // Poll elapsed time
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        match client.request(Request::GetElapsedTime).await {
            Ok(Response::ElapsedTime { seconds }) => {
                if !quiet && !json && seconds != last_elapsed {
                    last_elapsed = seconds;
                    let mins = seconds / 60;
                    let secs = seconds % 60;
                    print!("\r{} {}", colors::recording("Recording:"), colors::elapsed_time(mins, secs));
                    std::io::stdout().flush().ok();
                }
            }
            Ok(Response::RecordingState { state: RecordingState::Idle }) => {
                // Recording was stopped externally
                if !quiet && !json {
                    eprintln!("\n{}", colors::info("Recording stopped externally."));
                }
                return ExitCode::Success;
            }
            Err(ServiceError::RemoteError(msg)) if msg.contains("not recording") => {
                // Recording was stopped externally
                if !quiet && !json {
                    eprintln!("\n{}", colors::info("Recording stopped."));
                }
                return ExitCode::Success;
            }
            _ => {}
        }
    }

    // Stop recording
    match client.request(Request::StopRecording).await {
        Ok(Response::RecordingStopped {
            file_path,
            source_path: _,
        }) => {
            if json {
                println!(
                    r#"{{"status": "recording_stopped", "file_path": "{}"}}"#,
                    file_path.replace('\\', "\\\\").replace('"', "\\\"")
                );
            } else if !quiet {
                println!(
                    "\n{} {}",
                    colors::success("Recording saved:"),
                    colors::path(&file_path)
                );
            }
            ExitCode::Success
        }
        Ok(other) => {
            if !quiet {
                eprintln!("\n{}", colors::error(&format!("Unexpected response when stopping: {:?}", other)));
            }
            ExitCode::RecordingFailedDuringCapture
        }
        Err(e) => {
            if !quiet {
                eprintln!("\n{}", colors::error(&format!("Error stopping recording: {}", e)));
            }
            ExitCode::RecordingFailedDuringCapture
        }
    }
}

/// Stop the current recording.
pub async fn stop(json: bool, quiet: bool) -> ExitCode {
    let client = ServiceClient::new();

    if let Err(e) = client.connect_or_spawn().await {
        if !quiet {
            eprintln!("{}", colors::error(&e.to_string()));
        }
        return e.to_exit_code();
    }

    match client.request(Request::StopRecording).await {
        Ok(Response::RecordingStopped {
            file_path,
            source_path: _,
        }) => {
            if json {
                println!(
                    r#"{{"status": "stopped", "file_path": "{}"}}"#,
                    file_path.replace('\\', "\\\\").replace('"', "\\\"")
                );
            } else if !quiet {
                println!("{} {}", colors::success("Recording saved:"), colors::path(&file_path));
            }
            ExitCode::Success
        }
        Err(ServiceError::RemoteError(msg)) if msg.contains("not recording") => {
            if json {
                println!(r#"{{"status": "not_recording"}}"#);
            } else if !quiet {
                println!("{}", colors::dim("No recording in progress."));
            }
            ExitCode::Success
        }
        Ok(other) => {
            if !quiet {
                eprintln!("{}", colors::error(&format!("Unexpected response: {:?}", other)));
            }
            ExitCode::GeneralError
        }
        Err(e) => {
            if !quiet {
                eprintln!("{}", colors::error(&e.to_string()));
            }
            e.to_exit_code()
        }
    }
}

/// Show current recording status.
pub async fn status(json: bool) -> ExitCode {
    let client = ServiceClient::new();

    if let Err(e) = client.connect_or_spawn().await {
        if json {
            println!(r#"{{"status": "service_unavailable", "error": "{}"}}"#, e);
        } else {
            eprintln!("{}", colors::error(&e.to_string()));
        }
        return e.to_exit_code();
    }

    match client.request(Request::GetRecordingState).await {
        Ok(Response::RecordingState { state }) => {
            let state_str = match state {
                RecordingState::Idle => "idle",
                RecordingState::Recording => "recording",
                RecordingState::Saving => "saving",
            };

            if state == RecordingState::Recording {
                // Get elapsed time
                match client.request(Request::GetElapsedTime).await {
                    Ok(Response::ElapsedTime { seconds }) => {
                        if json {
                            println!(
                                r#"{{"state": "{}", "elapsed_seconds": {}}}"#,
                                state_str, seconds
                            );
                        } else {
                            let mins = seconds / 60;
                            let secs = seconds % 60;
                            println!("{} {}", colors::bold("State:"), colors::state(state_str));
                            println!("{} {}", colors::bold("Elapsed:"), colors::elapsed_time(mins, secs));
                        }
                    }
                    _ => {
                        if json {
                            println!(r#"{{"state": "{}"}}"#, state_str);
                        } else {
                            println!("{} {}", colors::bold("State:"), colors::state(state_str));
                        }
                    }
                }
            } else if json {
                println!(r#"{{"state": "{}"}}"#, state_str);
            } else {
                println!("{} {}", colors::bold("State:"), colors::state(state_str));
            }
            ExitCode::Success
        }
        Ok(other) => {
            if json {
                println!(r#"{{"error": "unexpected_response"}}"#);
            } else {
                eprintln!("{}", colors::error(&format!("Unexpected response: {:?}", other)));
            }
            ExitCode::GeneralError
        }
        Err(e) => {
            if json {
                println!(r#"{{"error": "{}"}}"#, e);
            } else {
                eprintln!("{}", colors::error(&e.to_string()));
            }
            e.to_exit_code()
        }
    }
}

/// Show version information.
pub fn version(json: bool) {
    let version = env!("CARGO_PKG_VERSION");
    if json {
        println!(r#"{{"version": "{}"}}"#, version);
    } else {
        println!("{} {}", colors::bold("omnirec"), version);
    }
}
