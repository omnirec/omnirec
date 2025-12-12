//! Linux platform capture implementation using Wayland/PipeWire.
//!
//! This module provides screen capture functionality on Linux through:
//! - Hyprland IPC for window/monitor enumeration
//! - xdg-desktop-portal for capture authorization
//! - PipeWire for video/audio streaming
//!
//! The capture flow involves a separate picker service that auto-approves
//! portal requests based on the user's selection in the main app UI.

pub mod highlight;
pub mod ipc_server;
pub mod pipewire_capture;
pub mod portal_client;
pub mod screencopy;
pub mod thumbnail;

use crate::capture::error::{CaptureError, EnumerationError};
use crate::capture::types::{
    CaptureRegion, FrameReceiver, MonitorInfo, StopHandle, WindowInfo,
};
use crate::capture::{CaptureBackend, HighlightProvider, MonitorEnumerator, ThumbnailCapture, ThumbnailResult, WindowEnumerator};

use hyprland::data::{Clients, Monitors};
use hyprland::shared::HyprData;
use std::sync::Arc;
use tokio::sync::RwLock;

use ipc_server::IpcServerState;

/// Global IPC server state (initialized once at startup)
static IPC_STATE: once_cell::sync::OnceCell<Arc<RwLock<IpcServerState>>> = once_cell::sync::OnceCell::new();

/// Initialize the global IPC server (call once at app startup).
pub async fn init_ipc_server() -> Result<(), String> {
    if IPC_STATE.get().is_some() {
        eprintln!("[Linux] IPC server already initialized");
        return Ok(());
    }

    eprintln!("[Linux] Starting IPC server...");
    let state = ipc_server::start_ipc_server()
        .await
        .map_err(|e| format!("Failed to start IPC server: {}", e))?;

    IPC_STATE.set(state).map_err(|_| "IPC state already set")?;
    eprintln!("[Linux] IPC server started at {:?}", ipc_server::get_socket_path());
    Ok(())
}

/// Get the global IPC state.
pub fn get_ipc_state() -> Option<Arc<RwLock<IpcServerState>>> {
    IPC_STATE.get().cloned()
}

/// Pre-initialize the screencopy subsystem for faster first thumbnail.
///
/// Call this at app startup (after IPC init) to avoid latency on first thumbnail.
/// This is a no-op if the compositor doesn't support wlr-screencopy.
pub fn init_screencopy() {
    let _ = screencopy::init();
}

/// Test the portal flow: set selection, call portal, log results.
/// This is for validating Phase 1 implementation.
pub async fn test_portal_flow(monitor_id: &str) -> Result<String, String> {
    eprintln!("[Linux] === Testing Portal Flow ===");
    eprintln!("[Linux] Monitor ID: {}", monitor_id);

    // Step 1: Ensure IPC server is running
    let ipc_state = get_ipc_state().ok_or_else(|| {
        "IPC server not initialized. Call init_ipc_server() first.".to_string()
    })?;
    eprintln!("[Linux] Step 1: IPC server is running");

    // Step 2: Set the selection
    let selection = ipc_server::CaptureSelection {
        source_type: "monitor".to_string(),
        source_id: monitor_id.to_string(),
        geometry: None,
    };
    ipc_server::set_selection(&ipc_state, selection).await;
    eprintln!("[Linux] Step 2: Selection set for monitor '{}'", monitor_id);

    // Step 3: Create portal client and request capture
    eprintln!("[Linux] Step 3: Requesting screencast via portal...");
    eprintln!("[Linux]   (This should trigger the picker service)");

    let portal_client = portal_client::PortalClient::new(ipc_state);
    
    match portal_client.request_monitor_capture(monitor_id).await {
        Ok(stream) => {
            let result = format!(
                "SUCCESS! Portal returned:\n  - PipeWire Node ID: {}\n  - Source Type: {:?}\n  - Size: {:?}",
                stream.node_id,
                stream.source_type,
                stream.size
            );
            eprintln!("[Linux] Step 4: {}", result);
            Ok(result)
        }
        Err(e) => {
            let result = format!("Portal request failed: {}", e);
            eprintln!("[Linux] Step 4: {}", result);
            Err(result)
        }
    }
}

/// Linux platform capture backend using Hyprland/PipeWire.
pub struct LinuxBackend {
    /// IPC server state for communicating with the picker service
    ipc_state: Option<Arc<RwLock<IpcServerState>>>,
}

impl LinuxBackend {
    /// Create a new Linux backend.
    pub fn new() -> Self {
        Self { ipc_state: None }
    }

    /// Check if running on Hyprland compositor.
    pub fn is_hyprland() -> bool {
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
    }

    /// Initialize the backend (starts IPC server).
    ///
    /// This should be called once at app startup on Linux.
    pub async fn initialize(&mut self) -> Result<(), String> {
        if !Self::is_hyprland() {
            return Err(
                "This application requires Hyprland compositor. \
                 HYPRLAND_INSTANCE_SIGNATURE not found."
                    .to_string(),
            );
        }

        // Start IPC server for picker communication
        let state = ipc_server::start_ipc_server()
            .await
            .map_err(|e| format!("Failed to start IPC server: {}", e))?;

        self.ipc_state = Some(state);
        Ok(())
    }

    /// Set the current capture selection (called before starting recording).
    pub async fn set_selection(&self, selection: ipc_server::CaptureSelection) -> Result<(), String> {
        match &self.ipc_state {
            Some(state) => {
                ipc_server::set_selection(state, selection).await;
                Ok(())
            }
            None => Err("IPC server not initialized".to_string()),
        }
    }

    /// Clear the current capture selection.
    pub async fn clear_selection(&self) -> Result<(), String> {
        match &self.ipc_state {
            Some(state) => {
                ipc_server::clear_selection(state).await;
                Ok(())
            }
            None => Err("IPC server not initialized".to_string()),
        }
    }
}

impl Default for LinuxBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowEnumerator for LinuxBackend {
    fn list_windows(&self) -> Result<Vec<WindowInfo>, EnumerationError> {
        if !Self::is_hyprland() {
            return Err(EnumerationError::NotImplemented(
                "Window enumeration requires Hyprland compositor".to_string(),
            ));
        }

        // Query Hyprland for client (window) list
        let clients = Clients::get().map_err(|e| {
            EnumerationError::PlatformError(format!("Failed to get Hyprland clients: {}", e))
        })?;

        // Get monitors to look up scale factors (Hyprland Monitor has numeric `id` field)
        let monitors = Monitors::get().ok();

        let mut windows = Vec::new();
        for client in clients {
            // Skip windows without titles or hidden windows
            if client.title.is_empty() {
                continue;
            }

            // Convert Hyprland address to isize handle
            // The address is a hex value like "0x5638d0a12345"
            let handle = client.address.to_string();
            let handle = handle.trim_start_matches("0x");
            let handle = isize::from_str_radix(handle, 16).unwrap_or(0);

            // Hyprland reports window position and size in logical (scaled) coordinates
            // We need to match the MonitorInfo format which uses:
            // - x, y: logical coordinates (Hyprland workspace coordinates)
            // - width, height: physical pixels
            let (logical_x, logical_y) = (client.at.0 as i32, client.at.1 as i32);
            let (logical_width, logical_height) = (client.size.0 as u32, client.size.1 as u32);

            // Find the scale factor for this window's monitor
            // client.monitor is Option<MonitorId> (i128), mon.id is also the numeric ID
            let scale = monitors.as_ref()
                .and_then(|m| {
                    client.monitor.and_then(|client_mon_id| {
                        m.iter().find(|mon| mon.id as i128 == client_mon_id)
                    })
                })
                .map(|mon| mon.scale as f64)
                .unwrap_or(1.0);

            // Convert only width/height to physical pixels (to match MonitorInfo format)
            // Keep x, y in logical coordinates (Hyprland workspace space)
            let width = (logical_width as f64 * scale).round() as u32;
            let height = (logical_height as f64 * scale).round() as u32;

            windows.push(WindowInfo {
                handle,
                title: client.title.clone(),
                process_name: client.class.clone(),
                x: logical_x,
                y: logical_y,
                width,
                height,
            });
        }

        Ok(windows)
    }
}

impl MonitorEnumerator for LinuxBackend {
    fn list_monitors(&self) -> Result<Vec<MonitorInfo>, EnumerationError> {
        if !Self::is_hyprland() {
            return Err(EnumerationError::NotImplemented(
                "Monitor enumeration requires Hyprland compositor".to_string(),
            ));
        }

        // Query Hyprland for monitor list
        let monitors = Monitors::get().map_err(|e| {
            EnumerationError::PlatformError(format!("Failed to get Hyprland monitors: {}", e))
        })?;

        let mut result = Vec::new();
        for monitor in monitors {
            eprintln!("[Linux] Monitor {}: {}x{} at ({},{}) scale={}", 
                monitor.name, monitor.width, monitor.height, monitor.x, monitor.y, monitor.scale);
            
            result.push(MonitorInfo {
                // Use monitor name as ID (e.g., "DP-1", "HDMI-A-1")
                id: monitor.name.clone(),
                // Display name includes description if available
                name: if monitor.description.is_empty() {
                    monitor.name.clone()
                } else {
                    format!("{} ({})", monitor.name, monitor.description)
                },
                x: monitor.x as i32,
                y: monitor.y as i32,
                width: monitor.width as u32,
                height: monitor.height as u32,
                is_primary: monitor.focused,
                scale_factor: monitor.scale as f64,
            });
        }

        Ok(result)
    }
}

impl CaptureBackend for LinuxBackend {
    fn start_window_capture(
        &self,
        window_handle: isize,
    ) -> Result<(FrameReceiver, StopHandle), CaptureError> {
        // Get window info to find the address
        let windows = self.list_windows().map_err(|e| {
            CaptureError::PlatformError(format!("Failed to list windows: {}", e))
        })?;
        
        let window = windows.iter().find(|w| w.handle == window_handle).ok_or_else(|| {
            CaptureError::TargetNotFound(format!("Window with handle {} not found", window_handle))
        })?;
        
        // The window handle is the address converted to isize, convert back to hex string
        let window_address = format!("0x{:x}", window_handle as usize);
        
        eprintln!("[Linux] Starting window capture for {} ({})", window.title, window_address);
        
        // Get IPC state
        let ipc_state = get_ipc_state().ok_or_else(|| {
            CaptureError::PlatformError("IPC server not initialized".to_string())
        })?;
        
        // Use block_in_place to run async code from sync context within tokio runtime
        let stream = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            let portal_client = portal_client::PortalClient::new(ipc_state);
            rt.block_on(portal_client.request_window_capture(&window_address))
        }).map_err(|e| CaptureError::PlatformError(e))?;
        
        eprintln!("[Linux] Portal returned node ID {} for window capture", stream.node_id);
        
        // Get window dimensions from Hyprland
        let (width, height) = stream.size.map(|(w, h)| (w as u32, h as u32))
            .unwrap_or((1920, 1080)); // Fallback dimensions
        
        // Start PipeWire capture
        pipewire_capture::start_pipewire_capture(stream.node_id, width, height)
            .map_err(|e| CaptureError::PlatformError(e))
    }

    fn start_region_capture(
        &self,
        region: CaptureRegion,
    ) -> Result<(FrameReceiver, StopHandle), CaptureError> {
        eprintln!("[Linux] Starting region capture for {} ({}x{} at {},{})", 
            region.monitor_id, region.width, region.height, region.x, region.y);
        
        // Validate region bounds
        if region.width == 0 || region.height == 0 {
            return Err(CaptureError::InvalidRegion(
                "Region width and height must be greater than 0".to_string()
            ));
        }
        
        // Check minimum size (100x100 per spec)
        if region.width < 100 || region.height < 100 {
            return Err(CaptureError::InvalidRegion(
                format!("Region must be at least 100x100 pixels (got {}x{})", region.width, region.height)
            ));
        }
        
        // Get monitor info to validate region and get full dimensions
        let monitors = self.list_monitors().map_err(|e| {
            CaptureError::PlatformError(format!("Failed to list monitors: {}", e))
        })?;
        
        let monitor = monitors.iter().find(|m| m.id == region.monitor_id).ok_or_else(|| {
            CaptureError::TargetNotFound(format!("Monitor '{}' not found", region.monitor_id))
        })?;
        
        // Validate region is within monitor bounds
        if region.x < 0 || region.y < 0 {
            return Err(CaptureError::InvalidRegion(
                format!("Region coordinates cannot be negative ({}, {})", region.x, region.y)
            ));
        }
        
        let region_x_end = region.x as u32 + region.width;
        let region_y_end = region.y as u32 + region.height;
        
        if region_x_end > monitor.width || region_y_end > monitor.height {
            return Err(CaptureError::InvalidRegion(
                format!("Region extends beyond monitor bounds (region: {}x{} at {},{}, monitor: {}x{})",
                    region.width, region.height, region.x, region.y, monitor.width, monitor.height)
            ));
        }
        
        // Get IPC state
        let ipc_state = get_ipc_state().ok_or_else(|| {
            CaptureError::PlatformError("IPC server not initialized".to_string())
        })?;
        
        // Use block_in_place to run async code from sync context within tokio runtime
        let monitor_id_clone = region.monitor_id.clone();
        let region_x = region.x;
        let region_y = region.y;
        let region_width = region.width;
        let region_height = region.height;
        
        let stream = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            let portal_client = portal_client::PortalClient::new(ipc_state);
            rt.block_on(portal_client.request_region_capture(
                &monitor_id_clone,
                region_x,
                region_y,
                region_width,
                region_height,
            ))
        }).map_err(|e| CaptureError::PlatformError(e))?;
        
        eprintln!("[Linux] Portal returned node ID {} for region capture", stream.node_id);
        
        // Use portal-reported dimensions if available, otherwise use monitor dimensions
        let (capture_width, capture_height) = stream.size
            .map(|(w, h)| (w as u32, h as u32))
            .unwrap_or((monitor.width, monitor.height));
        
        eprintln!("[Linux] Capture stream size: {}x{}", capture_width, capture_height);
        eprintln!("[Linux] Monitor reported size: {}x{}", monitor.width, monitor.height);
        eprintln!("[Linux] Region from UI: {}x{} at {},{}", 
            region.width, region.height, region.x, region.y);
        
        // Check if the portal already cropped the stream to the region
        // XDPH does portal-level cropping for region selections
        let is_precropped = capture_width < monitor.width || capture_height < monitor.height;
        
        if is_precropped {
            eprintln!("[Linux] Portal provided pre-cropped stream - using as-is (no app-level cropping)");
            
            // The stream is already the region - just capture it directly
            pipewire_capture::start_pipewire_capture(
                stream.node_id,
                capture_width,
                capture_height,
            )
            .map_err(|e| CaptureError::PlatformError(e))
        } else {
            eprintln!("[Linux] Portal provided full monitor stream - will crop in app");
            
            // We got the full monitor, need to crop ourselves
            // This shouldn't happen with XDPH region format, but handle it just in case
            let scale_x = capture_width as f64 / monitor.width as f64;
            let scale_y = capture_height as f64 / monitor.height as f64;
            
            let scaled_x = (region.x as f64 * scale_x).round() as i32;
            let scaled_y = (region.y as f64 * scale_y).round() as i32;
            let scaled_width = (region.width as f64 * scale_x).round() as u32;
            let scaled_height = (region.height as f64 * scale_y).round() as u32;
            
            eprintln!("[Linux] App-level crop region: {}x{} at {},{}", 
                scaled_width, scaled_height, scaled_x, scaled_y);
            
            let crop_region = pipewire_capture::CropRegion {
                x: scaled_x,
                y: scaled_y,
                width: scaled_width,
                height: scaled_height,
            };
            
            pipewire_capture::start_pipewire_capture_with_crop(
                stream.node_id,
                capture_width,
                capture_height,
                Some(crop_region),
            )
            .map_err(|e| CaptureError::PlatformError(e))
        }
    }

    fn start_display_capture(
        &self,
        monitor_id: String,
        width: u32,
        height: u32,
    ) -> Result<(FrameReceiver, StopHandle), CaptureError> {
        eprintln!("[Linux] Starting display capture for {} ({}x{})", monitor_id, width, height);
        
        // Get IPC state
        let ipc_state = get_ipc_state().ok_or_else(|| {
            CaptureError::PlatformError("IPC server not initialized".to_string())
        })?;
        
        // Use block_in_place to run async code from sync context within tokio runtime
        let monitor_id_clone = monitor_id.clone();
        let stream = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            let portal_client = portal_client::PortalClient::new(ipc_state);
            rt.block_on(portal_client.request_monitor_capture(&monitor_id_clone))
        }).map_err(|e| CaptureError::PlatformError(e))?;
        
        eprintln!("[Linux] Portal returned node ID {} for display capture", stream.node_id);
        
        // Use portal-reported dimensions if available, otherwise use provided ones
        let (capture_width, capture_height) = stream.size
            .map(|(w, h)| (w as u32, h as u32))
            .unwrap_or((width, height));
        
        // Start PipeWire capture
        pipewire_capture::start_pipewire_capture(stream.node_id, capture_width, capture_height)
            .map_err(|e| CaptureError::PlatformError(e))
    }
}

impl HighlightProvider for LinuxBackend {
    fn show_highlight(&self, x: i32, y: i32, width: i32, height: i32) {
        highlight::show_highlight(x, y, width, height);
    }
}

impl ThumbnailCapture for LinuxBackend {
    fn capture_window_thumbnail(&self, window_handle: isize) -> Result<ThumbnailResult, CaptureError> {
        let thumb_capture = thumbnail::LinuxThumbnailCapture::new();
        thumb_capture.capture_window_thumbnail(window_handle)
    }

    fn capture_display_thumbnail(&self, monitor_id: &str) -> Result<ThumbnailResult, CaptureError> {
        let thumb_capture = thumbnail::LinuxThumbnailCapture::new();
        thumb_capture.capture_display_thumbnail(monitor_id)
    }

    fn capture_region_preview(
        &self,
        monitor_id: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<ThumbnailResult, CaptureError> {
        let thumb_capture = thumbnail::LinuxThumbnailCapture::new();
        thumb_capture.capture_region_preview(monitor_id, x, y, width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_creation() {
        let backend = LinuxBackend::new();
        assert!(backend.ipc_state.is_none());
    }

    #[test]
    fn test_hyprland_detection() {
        // This test will pass/fail based on environment
        let _is_hyprland = LinuxBackend::is_hyprland();
    }
}
