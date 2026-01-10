//! IPC request handlers.
//!
//! This module contains handlers for each IPC request type.

use crate::capture;
use crate::state::get_recording_manager;
use omnirec_common::ipc::{Request, Response};
use omnirec_common::{AudioConfig, CaptureRegion, OutputFormat, TranscriptionConfig};
use tracing::{debug, error, info, warn};

/// Handle an IPC request and return a response.
pub async fn handle_request(request: Request) -> Response {
    debug!("Handling request: {:?}", request);

    match request {
        // === Enumeration ===
        Request::ListWindows => {
            let windows = capture::list_windows();
            info!("Listed {} windows", windows.len());
            Response::Windows { windows }
        }
        Request::ListMonitors => {
            let monitors = capture::list_monitors();
            info!("Listed {} monitors", monitors.len());
            Response::Monitors { monitors }
        }
        Request::ListAudioSources => {
            let sources = capture::list_audio_sources();
            info!("Listed {} audio sources", sources.len());
            Response::AudioSources { sources }
        }

        // === Capture Control ===
        Request::StartWindowCapture { window_handle } => {
            info!("StartWindowCapture: handle={}", window_handle);
            let manager = get_recording_manager();
            match manager.start_window_capture(window_handle).await {
                Ok(()) => Response::RecordingStarted,
                Err(e) => {
                    error!("Failed to start window capture: {}", e);
                    Response::error(e)
                }
            }
        }
        Request::StartDisplayCapture {
            monitor_id,
            width,
            height,
        } => {
            info!(
                "StartDisplayCapture: {}x{} on {}",
                width, height, monitor_id
            );
            let manager = get_recording_manager();
            match manager
                .start_display_capture(monitor_id, width, height)
                .await
            {
                Ok(()) => Response::RecordingStarted,
                Err(e) => {
                    error!("Failed to start display capture: {}", e);
                    Response::error(e)
                }
            }
        }
        Request::StartRegionCapture {
            monitor_id,
            x,
            y,
            width,
            height,
        } => {
            info!(
                "StartRegionCapture: {}x{} at ({},{}) on {}",
                width, height, x, y, monitor_id
            );
            let region = CaptureRegion {
                monitor_id,
                x,
                y,
                width,
                height,
            };
            let manager = get_recording_manager();
            match manager.start_region_capture(region).await {
                Ok(()) => Response::RecordingStarted,
                Err(e) => {
                    error!("Failed to start region capture: {}", e);
                    Response::error(e)
                }
            }
        }
        Request::StartPortalCapture => {
            info!("StartPortalCapture");
            let manager = get_recording_manager();
            match manager.start_portal_capture().await {
                Ok(()) => Response::RecordingStarted,
                Err(e) => {
                    error!("Failed to start portal capture: {}", e);
                    Response::error(e)
                }
            }
        }
        Request::StopRecording => {
            info!("StopRecording");
            let manager = get_recording_manager();
            match manager.stop_recording().await {
                Ok(result) => Response::RecordingStopped {
                    file_path: result.file_path.display().to_string(),
                    source_path: result.source_path.display().to_string(),
                },
                Err(e) => {
                    error!("Failed to stop recording: {}", e);
                    Response::error(e)
                }
            }
        }

        // === State Queries ===
        Request::GetRecordingState => {
            let manager = get_recording_manager();
            let state = manager.get_state().await;
            Response::RecordingState { state }
        }
        Request::GetElapsedTime => {
            let manager = get_recording_manager();
            let seconds = manager.get_elapsed_seconds().await;
            Response::ElapsedTime { seconds }
        }
        Request::SubscribeEvents => {
            // TODO: Implement event subscription via streaming
            // For now, just acknowledge subscription
            Response::Subscribed
        }

        // === Configuration ===
        Request::GetOutputFormat => {
            let manager = get_recording_manager();
            let format = manager.get_output_format().await;
            Response::OutputFormat {
                format: format!("{:?}", format).to_lowercase(),
            }
        }
        Request::SetOutputFormat { format } => {
            info!("SetOutputFormat: {}", format);
            let parsed = OutputFormat::parse(&format);
            match parsed {
                Some(fmt) => {
                    let manager = get_recording_manager();
                    match manager.set_output_format(fmt).await {
                        Ok(()) => Response::ok(),
                        Err(e) => Response::error(e),
                    }
                }
                None => Response::error(format!("Unknown output format: {}", format)),
            }
        }
        Request::GetAudioConfig => {
            let manager = get_recording_manager();
            let config = manager.get_audio_config().await;
            Response::AudioConfig(config)
        }
        Request::SetAudioConfig {
            enabled,
            source_id,
            microphone_id,
            echo_cancellation,
        } => {
            info!(
                "SetAudioConfig: enabled={}, source={:?}, mic={:?}, aec={}",
                enabled, source_id, microphone_id, echo_cancellation
            );
            let config = AudioConfig {
                enabled,
                source_id,
                microphone_id,
                echo_cancellation,
            };
            let manager = get_recording_manager();
            match manager.set_audio_config(config).await {
                Ok(()) => Response::ok(),
                Err(e) => Response::error(e),
            }
        }

        // === Thumbnails ===
        Request::GetWindowThumbnail { window_handle } => {
            use crate::capture::ThumbnailCapture;
            let backend = capture::get_backend();
            match backend.capture_window_thumbnail(window_handle) {
                Ok(result) => Response::Thumbnail {
                    data: result.data,
                    width: result.width,
                    height: result.height,
                },
                Err(e) => {
                    warn!("Failed to capture window thumbnail: {}", e);
                    Response::error(format!("Failed to capture thumbnail: {}", e))
                }
            }
        }
        Request::GetDisplayThumbnail { monitor_id } => {
            use crate::capture::ThumbnailCapture;
            let backend = capture::get_backend();
            match backend.capture_display_thumbnail(&monitor_id) {
                Ok(result) => Response::Thumbnail {
                    data: result.data,
                    width: result.width,
                    height: result.height,
                },
                Err(e) => {
                    warn!("Failed to capture display thumbnail: {}", e);
                    Response::error(format!("Failed to capture thumbnail: {}", e))
                }
            }
        }
        Request::GetRegionPreview {
            monitor_id,
            x,
            y,
            width,
            height,
        } => {
            use crate::capture::ThumbnailCapture;
            let backend = capture::get_backend();
            match backend.capture_region_preview(&monitor_id, x, y, width, height) {
                Ok(result) => Response::Thumbnail {
                    data: result.data,
                    width: result.width,
                    height: result.height,
                },
                Err(e) => {
                    warn!("Failed to capture region preview: {}", e);
                    Response::error(format!("Failed to capture preview: {}", e))
                }
            }
        }

        // === Highlights ===
        Request::ShowDisplayHighlight {
            x,
            y,
            width,
            height,
        } => {
            capture::show_highlight(x, y, width, height);
            Response::ok()
        }
        Request::ShowWindowHighlight { window_handle } => {
            // Get window geometry and show highlight
            let windows = capture::list_windows();
            if let Some(window) = windows.iter().find(|w| w.handle == window_handle) {
                capture::show_highlight(
                    window.x,
                    window.y,
                    window.width as i32,
                    window.height as i32,
                );
                Response::ok()
            } else {
                Response::error("Window not found")
            }
        }
        Request::ClearHighlight => {
            // Highlights auto-dismiss, nothing to do
            Response::ok()
        }

        // === Picker Compatibility ===
        Request::QuerySelection => {
            #[cfg(target_os = "linux")]
            {
                use crate::capture::linux::approval_token;
                use omnirec_common::ipc::SelectionGeometry;
                if let Some(state) = crate::capture::linux::get_ipc_state() {
                    let guard = state.read().await;
                    if let Some(ref selection) = guard.selection {
                        let geometry = selection.geometry.as_ref().map(|g| SelectionGeometry {
                            x: g.x,
                            y: g.y,
                            width: g.width,
                            height: g.height,
                        });
                        return Response::Selection {
                            source_type: selection.source_type.clone(),
                            source_id: selection.source_id.clone(),
                            has_approval_token: approval_token::has_token(),
                            geometry,
                        };
                    }
                }
            }
            Response::NoSelection
        }
        Request::ValidateToken { token } => {
            #[cfg(target_os = "linux")]
            {
                use crate::capture::linux::approval_token;
                if approval_token::validate_token(&token) {
                    return Response::TokenValid;
                }
            }
            #[cfg(not(target_os = "linux"))]
            let _ = token; // Silence unused variable warning on non-Linux
            Response::TokenInvalid
        }
        Request::StoreToken { token } => {
            #[cfg(target_os = "linux")]
            {
                use crate::capture::linux::approval_token;
                if let Err(e) = approval_token::write_token(&token) {
                    warn!("Failed to store approval token: {}", e);
                }
            }
            #[cfg(not(target_os = "linux"))]
            let _ = token; // Silence unused variable warning on non-Linux
            Response::TokenStored
        }

        // === Transcription ===
        Request::GetTranscriptionConfig => {
            let manager = get_recording_manager();
            let config = manager.get_transcription_config().await;
            Response::TranscriptionConfig(config)
        }
        Request::SetTranscriptionConfig {
            enabled,
            model_path,
        } => {
            info!(
                "SetTranscriptionConfig: enabled={}, model_path={:?}",
                enabled, model_path
            );
            let config = TranscriptionConfig {
                enabled,
                model_path,
            };
            let manager = get_recording_manager();
            match manager.set_transcription_config(config).await {
                Ok(()) => Response::ok(),
                Err(e) => Response::error(e),
            }
        }
        Request::GetTranscriptionStatus => {
            let manager = get_recording_manager();
            let status = manager.get_transcription_status().await;
            Response::TranscriptionStatus(status)
        }

        // === Service Control ===
        Request::Shutdown => {
            info!("Shutdown requested via IPC");
            // Trigger graceful shutdown
            crate::request_shutdown();
            Response::ok()
        }
        Request::Ping => Response::Pong,
    }
}
