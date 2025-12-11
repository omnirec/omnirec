//! Linux thumbnail capture implementation using PipeWire.
//!
//! This module captures single frames from windows and displays for use as
//! thumbnails in the UI. It uses the existing portal/PipeWire infrastructure
//! but captures only one frame before stopping.

use crate::capture::error::CaptureError;
use crate::capture::thumbnail::{
    bgra_to_jpeg_thumbnail, PREVIEW_MAX_HEIGHT, PREVIEW_MAX_WIDTH, THUMBNAIL_MAX_HEIGHT,
    THUMBNAIL_MAX_WIDTH,
};
use crate::capture::{MonitorEnumerator, ThumbnailCapture, ThumbnailResult, WindowEnumerator};

use super::ipc_server::IpcServerState;
use super::portal_client::PortalClient;
use super::pipewire_capture::CropRegion;
use super::{get_ipc_state, LinuxBackend};

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Capture a single frame from a PipeWire stream.
///
/// This starts a PipeWire capture, waits for one frame, then immediately stops.
async fn capture_single_frame(
    node_id: u32,
    width: u32,
    height: u32,
    crop_region: Option<CropRegion>,
) -> Result<(Vec<u8>, u32, u32), String> {
    use super::pipewire_capture;

    eprintln!("[Thumbnail] Starting PipeWire capture for node {} ({}x{})", node_id, width, height);

    // Start capture
    let (mut frame_rx, stop_flag) = if let Some(crop) = crop_region {
        pipewire_capture::start_pipewire_capture_with_crop(node_id, width, height, Some(crop))?
    } else {
        pipewire_capture::start_pipewire_capture(node_id, width, height)?
    };

    // Wait for a single frame with timeout
    let frame = tokio::time::timeout(Duration::from_secs(5), frame_rx.recv())
        .await
        .map_err(|_| "Timeout waiting for frame".to_string())?
        .ok_or_else(|| "No frame received".to_string())?;

    eprintln!("[Thumbnail] Got frame: {}x{}", frame.width, frame.height);

    // Stop capture immediately
    stop_flag.store(true, Ordering::SeqCst);

    Ok((frame.data, frame.width, frame.height))
}

/// Linux thumbnail capture implementation.
pub struct LinuxThumbnailCapture {
    ipc_state: Arc<RwLock<IpcServerState>>,
}

impl LinuxThumbnailCapture {
    /// Create a new Linux thumbnail capture instance.
    pub fn new(ipc_state: Arc<RwLock<IpcServerState>>) -> Self {
        Self { ipc_state }
    }

    /// Create from the global IPC state if available.
    pub fn from_global() -> Option<Self> {
        get_ipc_state().map(|state| Self::new(state))
    }
}

impl ThumbnailCapture for LinuxThumbnailCapture {
    fn capture_window_thumbnail(&self, window_handle: isize) -> Result<ThumbnailResult, CaptureError> {
        // Get window info to find the address
        let backend = LinuxBackend::new();
        let windows = backend.list_windows().map_err(|e| {
            CaptureError::PlatformError(format!("Failed to list windows: {}", e))
        })?;

        let window = windows
            .iter()
            .find(|w| w.handle == window_handle)
            .ok_or_else(|| {
                CaptureError::TargetNotFound(format!("Window with handle {} not found", window_handle))
            })?;

        // Convert handle to hex address
        let window_address = format!("0x{:x}", window_handle as usize);

        eprintln!(
            "[Thumbnail] Capturing window thumbnail for {} ({})",
            window.title, window_address
        );

        // Request capture via portal
        let ipc_state = self.ipc_state.clone();
        let stream = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            let portal_client = PortalClient::new(ipc_state);
            rt.block_on(portal_client.request_window_capture(&window_address))
        })
        .map_err(|e| CaptureError::PlatformError(e))?;

        // Capture single frame
        let (width, height) = stream
            .size
            .map(|(w, h)| (w as u32, h as u32))
            .unwrap_or((window.width, window.height));

        let (data, frame_width, frame_height) = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(capture_single_frame(stream.node_id, width, height, None))
        })
        .map_err(|e| CaptureError::PlatformError(e))?;

        // Convert to thumbnail
        let (base64_data, thumb_width, thumb_height) = bgra_to_jpeg_thumbnail(
            &data,
            frame_width,
            frame_height,
            THUMBNAIL_MAX_WIDTH,
            THUMBNAIL_MAX_HEIGHT,
        )
        .map_err(|e| CaptureError::PlatformError(e))?;

        Ok(ThumbnailResult {
            data: base64_data,
            width: thumb_width,
            height: thumb_height,
        })
    }

    fn capture_display_thumbnail(&self, monitor_id: &str) -> Result<ThumbnailResult, CaptureError> {
        eprintln!("[Thumbnail] Capturing display thumbnail for {}", monitor_id);

        // Get monitor info for dimensions
        let backend = LinuxBackend::new();
        let monitors = backend.list_monitors().map_err(|e| {
            CaptureError::PlatformError(format!("Failed to list monitors: {}", e))
        })?;

        let monitor = monitors
            .iter()
            .find(|m| m.id == monitor_id)
            .ok_or_else(|| {
                CaptureError::TargetNotFound(format!("Monitor '{}' not found", monitor_id))
            })?;

        // Request capture via portal
        let ipc_state = self.ipc_state.clone();
        let monitor_id_owned = monitor_id.to_string();
        let stream = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            let portal_client = PortalClient::new(ipc_state);
            rt.block_on(portal_client.request_monitor_capture(&monitor_id_owned))
        })
        .map_err(|e| CaptureError::PlatformError(e))?;

        // Capture single frame
        let (width, height) = stream
            .size
            .map(|(w, h)| (w as u32, h as u32))
            .unwrap_or((monitor.width, monitor.height));

        let (data, frame_width, frame_height) = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(capture_single_frame(stream.node_id, width, height, None))
        })
        .map_err(|e| CaptureError::PlatformError(e))?;

        // Convert to thumbnail
        let (base64_data, thumb_width, thumb_height) = bgra_to_jpeg_thumbnail(
            &data,
            frame_width,
            frame_height,
            THUMBNAIL_MAX_WIDTH,
            THUMBNAIL_MAX_HEIGHT,
        )
        .map_err(|e| CaptureError::PlatformError(e))?;

        Ok(ThumbnailResult {
            data: base64_data,
            width: thumb_width,
            height: thumb_height,
        })
    }

    fn capture_region_preview(
        &self,
        monitor_id: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<ThumbnailResult, CaptureError> {
        eprintln!(
            "[Thumbnail] Capturing region preview for {} ({}x{} at {},{})",
            monitor_id, width, height, x, y
        );

        // Validate region
        if width < 100 || height < 100 {
            return Err(CaptureError::InvalidRegion(format!(
                "Region must be at least 100x100 pixels (got {}x{})",
                width, height
            )));
        }

        // Get monitor info
        let backend = LinuxBackend::new();
        let monitors = backend.list_monitors().map_err(|e| {
            CaptureError::PlatformError(format!("Failed to list monitors: {}", e))
        })?;

        let monitor = monitors
            .iter()
            .find(|m| m.id == monitor_id)
            .ok_or_else(|| {
                CaptureError::TargetNotFound(format!("Monitor '{}' not found", monitor_id))
            })?;

        // Request capture via portal (region capture)
        let ipc_state = self.ipc_state.clone();
        let monitor_id_owned = monitor_id.to_string();
        let stream = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            let portal_client = PortalClient::new(ipc_state);
            rt.block_on(portal_client.request_region_capture(
                &monitor_id_owned,
                x,
                y,
                width,
                height,
            ))
        })
        .map_err(|e| CaptureError::PlatformError(e))?;

        // Determine if portal pre-cropped the stream
        let (capture_width, capture_height) = stream
            .size
            .map(|(w, h)| (w as u32, h as u32))
            .unwrap_or((monitor.width, monitor.height));

        let is_precropped = capture_width < monitor.width || capture_height < monitor.height;

        let (data, frame_width, frame_height) = if is_precropped {
            // Portal already cropped - capture as-is
            tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(capture_single_frame(stream.node_id, capture_width, capture_height, None))
            })
            .map_err(|e| CaptureError::PlatformError(e))?
        } else {
            // Need to crop in app
            let crop = CropRegion {
                x,
                y,
                width,
                height,
            };
            tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(capture_single_frame(stream.node_id, capture_width, capture_height, Some(crop)))
            })
            .map_err(|e| CaptureError::PlatformError(e))?
        };

        // Convert to preview (larger than thumbnail)
        let (base64_data, preview_width, preview_height) = bgra_to_jpeg_thumbnail(
            &data,
            frame_width,
            frame_height,
            PREVIEW_MAX_WIDTH,
            PREVIEW_MAX_HEIGHT,
        )
        .map_err(|e| CaptureError::PlatformError(e))?;

        Ok(ThumbnailResult {
            data: base64_data,
            width: preview_width,
            height: preview_height,
        })
    }
}
