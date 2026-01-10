//! Portal client for initiating screencast requests.
//!
//! This module uses ashpd to communicate with xdg-desktop-portal for screen capture.
//! The portal request is handled by our custom picker service which auto-approves
//! based on the selection stored via IPC.

use ashpd::desktop::screencast::{CursorMode, Screencast, SourceType};
use ashpd::desktop::PersistMode;
use ashpd::enumflags2::BitFlags;
use ashpd::WindowIdentifier;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::ipc_server::{CaptureSelection, Geometry, IpcServerState};

/// Source type for capture selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureSourceType {
    Monitor,
    Window,
    Region,
}

impl CaptureSourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CaptureSourceType::Monitor => "monitor",
            CaptureSourceType::Window => "window",
            CaptureSourceType::Region => "region",
        }
    }
}

/// Result of a successful portal screencast request.
#[derive(Debug)]
pub struct ScreencastStream {
    /// PipeWire node ID for the video stream
    pub node_id: u32,
    /// Source type that was approved (e.g., Monitor, Window)
    #[allow(dead_code)]
    pub source_type: Option<SourceType>,
    /// Stream dimensions (if available) - this is the size of the content
    pub size: Option<(i32, i32)>,
    /// Position in compositor coordinates (for window captures)
    pub position: Option<(i32, i32)>,
}

/// Portal client for screen capture.
pub struct PortalClient {
    /// Reference to the IPC server state for setting selection
    ipc_state: Arc<RwLock<IpcServerState>>,
}

impl PortalClient {
    /// Create a new portal client.
    pub fn new(ipc_state: Arc<RwLock<IpcServerState>>) -> Self {
        Self { ipc_state }
    }

    /// Request a screencast stream for a monitor.
    ///
    /// This sets the selection in IPC state, then triggers the portal flow.
    /// Our picker service will query the selection and auto-approve.
    pub async fn request_monitor_capture(
        &self,
        monitor_id: &str,
    ) -> Result<ScreencastStream, String> {
        eprintln!("[PortalClient] request_monitor_capture: {}", monitor_id);

        // Set selection for picker to query
        let selection = CaptureSelection {
            source_type: CaptureSourceType::Monitor.as_str().to_string(),
            source_id: monitor_id.to_string(),
            geometry: None,
        };

        {
            eprintln!("[PortalClient] Setting IPC selection...");
            let mut state = self.ipc_state.write().await;
            state.selection = Some(selection);
            eprintln!("[PortalClient] IPC selection set");
        }

        // Initiate portal request
        eprintln!("[PortalClient] Initiating portal request...");
        self.request_screencast(SourceType::Monitor).await
    }

    /// Request a screencast stream for a window.
    pub async fn request_window_capture(
        &self,
        window_address: &str,
    ) -> Result<ScreencastStream, String> {
        eprintln!("[PortalClient] request_window_capture: {}", window_address);

        let selection = CaptureSelection {
            source_type: CaptureSourceType::Window.as_str().to_string(),
            source_id: window_address.to_string(),
            geometry: None,
        };

        {
            eprintln!("[PortalClient] Setting IPC selection...");
            let mut state = self.ipc_state.write().await;
            state.selection = Some(selection);
            eprintln!("[PortalClient] IPC selection set");
        }

        eprintln!("[PortalClient] Initiating portal request...");
        self.request_screencast(SourceType::Window).await
    }

    /// Request a screencast stream for a region.
    ///
    /// Region capture works by capturing the full monitor and cropping.
    pub async fn request_region_capture(
        &self,
        monitor_id: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<ScreencastStream, String> {
        let selection = CaptureSelection {
            source_type: CaptureSourceType::Region.as_str().to_string(),
            source_id: monitor_id.to_string(),
            geometry: Some(Geometry {
                x,
                y,
                width,
                height,
            }),
        };

        {
            let mut state = self.ipc_state.write().await;
            state.selection = Some(selection);
        }

        // Region capture uses monitor source type - app will crop the stream
        self.request_screencast(SourceType::Monitor).await
    }

    /// Request a screencast stream using the portal's native picker.
    ///
    /// This method does NOT set any IPC selection, allowing the portal's
    /// native picker (e.g., GNOME's dialog) to handle source selection.
    /// Use this for GNOME mode where we want the standard portal UX.
    pub async fn request_screencast_with_picker(&self) -> Result<ScreencastStream, String> {
        eprintln!("[PortalClient] request_screencast_with_picker: using native picker");

        // Clear any existing IPC selection so our custom picker doesn't interfere
        {
            let mut state = self.ipc_state.write().await;
            state.selection = None;
        }

        // Request with both Monitor and Window source types enabled
        // This allows the portal picker to offer all options
        self.request_screencast_multi(SourceType::Monitor | SourceType::Window)
            .await
    }

    /// Internal method to execute the portal screencast flow.
    async fn request_screencast(
        &self,
        source_type: SourceType,
    ) -> Result<ScreencastStream, String> {
        self.request_screencast_multi(source_type.into()).await
    }

    /// Internal method to execute the portal screencast flow with multiple source types.
    async fn request_screencast_multi(
        &self,
        source_types: BitFlags<SourceType>,
    ) -> Result<ScreencastStream, String> {
        eprintln!("[Portal] request_screencast_multi: connecting to portal...");

        // Get the screencast portal proxy
        let screencast = Screencast::new()
            .await
            .map_err(|e| format!("Failed to connect to screencast portal: {}", e))?;

        eprintln!("[Portal] Connected to screencast portal, creating session...");

        // Create a session
        let session = screencast
            .create_session()
            .await
            .map_err(|e| format!("Failed to create portal session: {}", e))?;

        eprintln!(
            "[Portal] Session created, selecting sources (types: {:?})...",
            source_types
        );

        // Select sources - this triggers the picker
        screencast
            .select_sources(
                &session,
                CursorMode::Embedded, // Include cursor in the capture
                source_types,
                false,              // multiple sources
                None,               // restore token
                PersistMode::DoNot, // don't persist for now
            )
            .await
            .map_err(|e| format!("Failed to select sources: {}", e))?;

        eprintln!("[Portal] Sources selected, starting screencast (picker should appear now)...");
        eprintln!("[Portal] NOTE: On KDE, check if the dialog appeared behind other windows or in the system tray");

        // Start the screencast - picker will handle source selection
        // Use None for parent window - this tells the portal we don't have a parent window
        // which should make the dialog appear as a top-level window
        let response = screencast
            .start(&session, &WindowIdentifier::None)
            .await
            .map_err(|e| format!("Failed to start screencast: {}", e))?;

        eprintln!("[Portal] Screencast start returned, waiting for response...");

        // Wait for the response
        let streams = response
            .response()
            .map_err(|e| format!("Portal request failed: {}", e))?;

        // Get the first stream
        let all_streams = streams.streams();
        eprintln!("[Portal] Got {} streams from portal", all_streams.len());

        let stream = all_streams
            .first()
            .ok_or_else(|| "No streams returned from portal".to_string())?;

        let node_id = stream.pipe_wire_node_id();
        let source_type = stream.source_type();
        let size = stream.size();
        let position = stream.position();

        eprintln!(
            "[Portal] Stream info: node_id={}, source_type={:?}, size={:?}, position={:?}",
            node_id, source_type, size, position
        );

        Ok(ScreencastStream {
            node_id,
            source_type,
            size,
            position,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_type_as_str() {
        assert_eq!(CaptureSourceType::Monitor.as_str(), "monitor");
        assert_eq!(CaptureSourceType::Window.as_str(), "window");
        assert_eq!(CaptureSourceType::Region.as_str(), "region");
    }
}
