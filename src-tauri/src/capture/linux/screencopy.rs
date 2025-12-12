//! Linux screenshot capture using wlr-screencopy protocol.
//!
//! This module provides efficient single-frame capture using the wlr-screencopy
//! Wayland protocol, which is much faster than the portal/PipeWire flow for
//! thumbnail generation.
//!
//! A persistent Wayland connection is maintained to avoid reconnection overhead.

use std::os::fd::AsRawFd;
use std::os::unix::io::{AsFd, OwnedFd};
use std::sync::Mutex;

use once_cell::sync::Lazy;
use wayland_client::protocol::{wl_buffer, wl_output, wl_registry, wl_shm, wl_shm_pool};
use wayland_client::{delegate_noop, Connection, Dispatch, EventQueue, QueueHandle, Proxy};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1, zwlr_screencopy_manager_v1,
};

/// Result of a screencopy capture operation.
pub struct ScreencopyFrame {
    /// Raw frame data in BGRA format
    pub data: Vec<u8>,
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
}

/// Information about a Wayland output for screencopy.
#[derive(Clone)]
struct OutputInfo {
    output: wl_output::WlOutput,
    name: String,
    width: u32,
    height: u32,
    done: bool,
}

/// Persistent Wayland connection state.
struct WaylandConnection {
    #[allow(dead_code)]
    conn: Connection, // Keep connection alive
    event_queue: EventQueue<ScreencopyState>,
    state: ScreencopyState,
}

/// Global cached Wayland connection.
static WAYLAND_CONNECTION: Lazy<Mutex<Option<WaylandConnection>>> = Lazy::new(|| Mutex::new(None));

/// State for screencopy capture.
struct ScreencopyState {
    shm: Option<wl_shm::WlShm>,
    screencopy_manager: Option<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1>,
    outputs: Vec<OutputInfo>,
    
    // Frame capture state (reset per capture)
    frame_format: Option<wl_shm::Format>,
    frame_width: u32,
    frame_height: u32,
    frame_stride: u32,
    frame_ready: bool,
    frame_failed: bool,
}

impl ScreencopyState {
    fn new() -> Self {
        Self {
            shm: None,
            screencopy_manager: None,
            outputs: Vec::new(),
            frame_format: None,
            frame_width: 0,
            frame_height: 0,
            frame_stride: 0,
            frame_ready: false,
            frame_failed: false,
        }
    }
    
    /// Reset frame capture state for a new capture.
    fn reset_frame_state(&mut self) {
        self.frame_format = None;
        self.frame_width = 0;
        self.frame_height = 0;
        self.frame_stride = 0;
        self.frame_ready = false;
        self.frame_failed = false;
    }
    
    /// Find output by name (monitor ID like "DP-1").
    fn find_output_by_name(&self, name: &str) -> Option<&OutputInfo> {
        self.outputs.iter().find(|o| o.done && o.name == name)
    }
}

/// Pre-initialize the Wayland connection.
///
/// Call this at app startup to avoid first-capture latency.
/// This is a no-op if already initialized or if wlr-screencopy is not available.
pub fn init() -> Result<(), String> {
    drop(get_or_init_connection()?);
    Ok(())
}

/// Initialize or get the cached Wayland connection.
fn get_or_init_connection() -> Result<std::sync::MutexGuard<'static, Option<WaylandConnection>>, String> {
    let mut guard = WAYLAND_CONNECTION.lock().map_err(|e| format!("Lock poisoned: {}", e))?;
    
    if guard.is_none() {
        
        // Connect to Wayland display
        let conn = Connection::connect_to_env()
            .map_err(|e| format!("Failed to connect to Wayland display: {}", e))?;
        
        let mut event_queue = conn.new_event_queue();
        let qh = event_queue.handle();
        
        // Get the display and create registry
        let display = conn.display();
        display.get_registry(&qh, ());
        
        // Initial state
        let mut state = ScreencopyState::new();
        
        // Roundtrip to get globals
        event_queue.roundtrip(&mut state)
            .map_err(|e| format!("Wayland roundtrip failed: {}", e))?;
        
        // Another roundtrip to get output info
        event_queue.roundtrip(&mut state)
            .map_err(|e| format!("Wayland roundtrip failed: {}", e))?;
        
        // Verify we have what we need
        if state.screencopy_manager.is_none() {
            return Err("Compositor does not support wlr-screencopy protocol".to_string());
        }
        
        if state.shm.is_none() {
            return Err("SHM global not found".to_string());
        }
        
        *guard = Some(WaylandConnection {
            conn,
            event_queue,
            state,
        });
    }
    
    Ok(guard)
}

/// Capture a screenshot of an output using wlr-screencopy.
///
/// Returns the frame data in BGRA format.
pub fn capture_output(monitor_name: &str) -> Result<ScreencopyFrame, String> {
    let mut guard = get_or_init_connection()?;
    let wl_conn = guard.as_mut().ok_or("Connection not initialized")?;
    
    let qh = wl_conn.event_queue.handle();
    
    // Reset frame state for new capture
    wl_conn.state.reset_frame_state();
    
    // Find the target output
    let output = wl_conn.state.find_output_by_name(monitor_name)
        .ok_or_else(|| format!("Output '{}' not found", monitor_name))?
        .output.clone();
    
    // Get globals we need
    let screencopy_manager = wl_conn.state.screencopy_manager.clone()
        .ok_or("Screencopy manager not available")?;
    let shm = wl_conn.state.shm.clone()
        .ok_or("SHM not available")?;
    
    // Request a frame capture (overlay_cursor = 1 to include cursor)
    let frame = screencopy_manager.capture_output(1, &output, &qh, ());
    
    // Roundtrip to get buffer event with format/size
    wl_conn.event_queue.roundtrip(&mut wl_conn.state)
        .map_err(|e| format!("Wayland roundtrip failed: {}", e))?;
    
    // Check if we got buffer info
    if wl_conn.state.frame_format.is_none() {
        frame.destroy();
        return Err("Did not receive buffer format from compositor".to_string());
    }
    
    let format = wl_conn.state.frame_format.unwrap();
    let width = wl_conn.state.frame_width;
    let height = wl_conn.state.frame_height;
    let stride = wl_conn.state.frame_stride;
    let size = (stride * height) as usize;
    
    // Create SHM buffer
    let fd = create_shm_fd(size)?;
    
    // Create SHM pool and buffer
    let pool = shm.create_pool(fd.as_fd(), size as i32, &qh, ());
    let buffer = pool.create_buffer(
        0,
        width as i32,
        height as i32,
        stride as i32,
        format,
        &qh,
        (),
    );
    
    // Copy frame to buffer
    frame.copy(&buffer);
    
    // Wait for ready or failed event
    while !wl_conn.state.frame_ready && !wl_conn.state.frame_failed {
        wl_conn.event_queue.blocking_dispatch(&mut wl_conn.state)
            .map_err(|e| format!("Dispatch failed: {}", e))?;
    }
    
    if wl_conn.state.frame_failed {
        frame.destroy();
        buffer.destroy();
        pool.destroy();
        return Err("Frame capture failed".to_string());
    }
    
    // Read the frame data from shared memory
    let frame_data = unsafe {
        let ptr = libc::mmap(
            std::ptr::null_mut(),
            size,
            libc::PROT_READ,
            libc::MAP_SHARED,
            fd.as_fd().as_raw_fd(),
            0,
        );
        
        if ptr == libc::MAP_FAILED {
            frame.destroy();
            buffer.destroy();
            pool.destroy();
            return Err("mmap for read failed".to_string());
        }
        
        let mut data = vec![0u8; size];
        std::ptr::copy_nonoverlapping(ptr as *const u8, data.as_mut_ptr(), size);
        
        libc::munmap(ptr, size);
        data
    };
    
    // Convert to BGRA if needed
    let bgra_data = convert_to_bgra(&frame_data, width, height, stride, format)?;
    
    // Cleanup
    frame.destroy();
    buffer.destroy();
    pool.destroy();
    
    Ok(ScreencopyFrame {
        data: bgra_data,
        width,
        height,
    })
}

/// Create a file descriptor for shared memory.
fn create_shm_fd(size: usize) -> Result<OwnedFd, String> {
    use std::os::fd::FromRawFd;
    
    let name = std::ffi::CString::new("omnirec-screencopy").unwrap();
    let fd = unsafe {
        libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC)
    };
    
    if fd < 0 {
        return Err("memfd_create failed".to_string());
    }
    
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    
    if unsafe { libc::ftruncate(fd.as_fd().as_raw_fd(), size as i64) } < 0 {
        return Err("ftruncate failed".to_string());
    }
    
    Ok(fd)
}

/// Convert frame data to BGRA format.
fn convert_to_bgra(
    data: &[u8],
    width: u32,
    height: u32,
    stride: u32,
    format: wl_shm::Format,
) -> Result<Vec<u8>, String> {
    let pixel_count = (width * height) as usize;
    let mut bgra = vec![0u8; pixel_count * 4];
    
    match format {
        // Already BGRA or BGRx - just copy (removing stride padding if any)
        wl_shm::Format::Argb8888 | wl_shm::Format::Xrgb8888 => {
            // These are actually BGRA in memory on little-endian systems
            for y in 0..height as usize {
                let src_offset = y * stride as usize;
                let dst_offset = y * width as usize * 4;
                bgra[dst_offset..dst_offset + width as usize * 4]
                    .copy_from_slice(&data[src_offset..src_offset + width as usize * 4]);
            }
        }
        wl_shm::Format::Abgr8888 | wl_shm::Format::Xbgr8888 => {
            // RGBA in memory - swap R and B
            for y in 0..height as usize {
                for x in 0..width as usize {
                    let src_idx = y * stride as usize + x * 4;
                    let dst_idx = (y * width as usize + x) * 4;
                    bgra[dst_idx] = data[src_idx + 2];     // B <- R
                    bgra[dst_idx + 1] = data[src_idx + 1]; // G
                    bgra[dst_idx + 2] = data[src_idx];     // R <- B
                    bgra[dst_idx + 3] = data[src_idx + 3]; // A
                }
            }
        }
        _ => {
            return Err(format!("Unsupported pixel format: {:?}", format));
        }
    }
    
    Ok(bgra)
}

// Wayland dispatch implementations

impl Dispatch<wl_registry::WlRegistry, ()> for ScreencopyState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_shm" => {
                    let shm = registry.bind::<wl_shm::WlShm, _, _>(
                        name,
                        version.min(1),
                        qh,
                        (),
                    );
                    state.shm = Some(shm);
                }
                "zwlr_screencopy_manager_v1" => {
                    let manager = registry.bind::<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, _, _>(
                        name,
                        version.min(3),
                        qh,
                        (),
                    );
                    state.screencopy_manager = Some(manager);
                }
                "wl_output" => {
                    let output = registry.bind::<wl_output::WlOutput, _, _>(
                        name,
                        version.min(4),
                        qh,
                        name,
                    );
                    state.outputs.push(OutputInfo {
                        output,
                        name: String::new(),
                        width: 0,
                        height: 0,
                        done: false,
                    });
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_output::WlOutput, u32> for ScreencopyState {
    fn event(
        state: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        _data: &u32,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let output_info = state.outputs.iter_mut().find(|o| o.output.id() == output.id());
        
        if let Some(info) = output_info {
            match event {
                wl_output::Event::Mode { width, height, flags, .. } => {
                    use wayland_client::WEnum;
                    if let WEnum::Value(mode_flags) = flags {
                        if mode_flags.contains(wl_output::Mode::Current) {
                            info.width = width as u32;
                            info.height = height as u32;
                        }
                    }
                }
                wl_output::Event::Name { name } => {
                    info.name = name;
                }
                wl_output::Event::Done => {
                    info.done = true;
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, ()> for ScreencopyState {
    fn event(
        state: &mut Self,
        _frame: &zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer { format, width, height, stride } => {
                use wayland_client::WEnum;
                if let WEnum::Value(fmt) = format {
                    state.frame_format = Some(fmt);
                    state.frame_width = width;
                    state.frame_height = height;
                    state.frame_stride = stride;
                }
            }
            zwlr_screencopy_frame_v1::Event::BufferDone => {
                // Buffer info complete
            }
            zwlr_screencopy_frame_v1::Event::Ready { .. } => {
                state.frame_ready = true;
            }
            zwlr_screencopy_frame_v1::Event::Failed => {
                state.frame_failed = true;
            }
            _ => {}
        }
    }
}

// No-op dispatchers for objects we don't need events from
delegate_noop!(ScreencopyState: ignore wl_shm::WlShm);
delegate_noop!(ScreencopyState: ignore wl_shm_pool::WlShmPool);
delegate_noop!(ScreencopyState: ignore wl_buffer::WlBuffer);
delegate_noop!(ScreencopyState: ignore zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1);

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_capture_output_no_display() {
        // This test verifies error handling when not connected to Wayland
        // In a real Wayland environment, it would actually capture
        let result = capture_output("nonexistent-output");
        // Either fails to connect or fails to find output - both are expected
        assert!(result.is_err());
    }
}
