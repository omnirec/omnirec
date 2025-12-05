//! Linux platform capture implementation using Wayland/PipeWire.
//!
//! This module provides screen capture functionality on Linux through:
//! - Hyprland IPC for window/monitor enumeration
//! - xdg-desktop-portal for capture authorization
//! - PipeWire for video/audio streaming (Phase 2)
//!
//! The capture flow involves a separate picker service that auto-approves
//! portal requests based on the user's selection in the main app UI.

pub mod ipc_server;
pub mod pipewire_capture;
pub mod portal_client;

use crate::capture::error::{CaptureError, EnumerationError};
use crate::capture::types::{
    CaptureRegion, FrameReceiver, MonitorInfo, StopHandle, WindowInfo,
};
use crate::capture::{CaptureBackend, HighlightProvider, MonitorEnumerator, WindowEnumerator};

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

            windows.push(WindowInfo {
                handle,
                title: client.title.clone(),
                process_name: client.class.clone(),
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
        _region: CaptureRegion,
    ) -> Result<(FrameReceiver, StopHandle), CaptureError> {
        // Phase 3: Implement region capture
        Err(CaptureError::NotImplemented(
            "Linux region capture will be implemented in Phase 3".to_string(),
        ))
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
    fn show_highlight(&self, _x: i32, _y: i32, _width: i32, _height: i32) {
        // Highlight is less important on Wayland - portal handles selection
        // Could potentially use layer-shell in the future
        eprintln!("Linux display highlight not yet implemented");
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
