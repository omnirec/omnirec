//! Linux display highlight using wlr-layer-shell protocol.
//!
//! Creates a transparent overlay surface with a colored border to highlight a monitor or window.
//! Uses the wlr-layer-shell Wayland protocol for broad compositor support.

use std::os::unix::io::{AsFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use hyprland::data::Monitors;
use hyprland::shared::HyprData;
use wayland_client::protocol::{wl_buffer, wl_compositor, wl_output, wl_registry, wl_shm, wl_shm_pool, wl_surface};
use wayland_client::{delegate_noop, Connection, Dispatch, QueueHandle, Proxy};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

const BORDER_WIDTH: i32 = 8;
const HIGHLIGHT_DURATION_MS: u64 = 800;

// Color: #2196F3 (blue) - same as Windows/macOS
const BORDER_R: u8 = 0x21;
const BORDER_G: u8 = 0x96;
const BORDER_B: u8 = 0xF3;
const BORDER_A: u8 = 0xFF;

/// Global flag to signal cancellation of a running highlight
static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);

/// Show a highlight border around the specified area.
/// This function spawns a thread and returns immediately.
pub fn show_highlight(x: i32, y: i32, width: i32, height: i32) {
    // Signal any existing highlight to cancel
    CANCEL_FLAG.store(true, Ordering::SeqCst);
    
    // Small delay to allow previous highlight to clean up
    thread::sleep(Duration::from_millis(10));
    
    // Reset cancel flag for new highlight
    CANCEL_FLAG.store(false, Ordering::SeqCst);
    
    thread::spawn(move || {
        if let Err(e) = run_highlight(x, y, width, height) {
            eprintln!("[Linux Highlight] Error: {}", e);
        }
    });
}

/// Information about a Wayland output
#[derive(Clone)]
struct OutputInfo {
    output: wl_output::WlOutput,
    name: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    scale: f64,
    done: bool,
}

/// Monitor info from Hyprland IPC
struct HyprMonitorInfo {
    name: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    scale: f64,
}

/// Get monitor positions from Hyprland IPC (more reliable than wl_output)
fn get_hyprland_monitors() -> Vec<HyprMonitorInfo> {
    match Monitors::get() {
        Ok(monitors) => {
            monitors.iter().map(|m| HyprMonitorInfo {
                name: m.name.clone(),
                x: m.x as i32,
                y: m.y as i32,
                width: m.width as i32,
                height: m.height as i32,
                scale: m.scale as f64,
            }).collect()
        }
        Err(e) => {
            eprintln!("[Linux Highlight] Failed to get Hyprland monitors: {}", e);
            Vec::new()
        }
    }
}

/// State for the highlight overlay
struct HighlightState {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    outputs: Vec<OutputInfo>,
    surface: Option<wl_surface::WlSurface>,
    layer_surface: Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    configured: bool,
    closed: bool,
}

impl HighlightState {
    fn new() -> Self {
        Self {
            compositor: None,
            shm: None,
            layer_shell: None,
            outputs: Vec::new(),
            surface: None,
            layer_surface: None,
            configured: false,
            closed: false,
        }
    }
    
    /// Update output positions from Hyprland IPC (wl_output positions are unreliable)
    fn update_output_positions_from_hyprland(&mut self) {
        let hypr_monitors = get_hyprland_monitors();
        
        // Match wl_outputs to Hyprland monitors by name
        for output in &mut self.outputs {
            if let Some(hypr) = hypr_monitors.iter().find(|h| h.name == output.name) {
                output.x = hypr.x;
                output.y = hypr.y;
                output.width = hypr.width;
                output.height = hypr.height;
                output.scale = hypr.scale;
            }
        }
    }
    
    /// Find the output that contains the given point
    /// Note: x, y are in Hyprland's coordinate space (logical/scaled)
    /// The output positions (o.x, o.y) are also in logical space from Hyprland
    /// But o.width/o.height are physical pixels, so we need to convert them
    fn find_output_for_point(&self, x: i32, y: i32) -> Option<&OutputInfo> {
        self.outputs.iter().find(|o| {
            // Convert physical dimensions to logical for comparison
            let logical_width = (o.width as f64 / o.scale).round() as i32;
            let logical_height = (o.height as f64 / o.scale).round() as i32;
            o.done && x >= o.x && x < o.x + logical_width && y >= o.y && y < o.y + logical_height
        })
    }
}

fn run_highlight(x: i32, y: i32, width: i32, height: i32) -> Result<(), String> {
    // Connect to Wayland display
    let conn = Connection::connect_to_env()
        .map_err(|e| format!("Failed to connect to Wayland display: {}", e))?;
    
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();
    
    // Get the display and create registry
    let display = conn.display();
    display.get_registry(&qh, ());
    
    // Initial state
    let mut state = HighlightState::new();
    
    // Roundtrip to get globals
    event_queue.roundtrip(&mut state)
        .map_err(|e| format!("Wayland roundtrip failed: {}", e))?;
    
    // Another roundtrip to get output geometry info
    event_queue.roundtrip(&mut state)
        .map_err(|e| format!("Wayland roundtrip failed: {}", e))?;
    
    // Update output positions from Hyprland IPC (wl_output positions are unreliable)
    state.update_output_positions_from_hyprland();
    
    // Check if layer shell is available
    if state.layer_shell.is_none() {
        return Err("Compositor does not support wlr-layer-shell protocol. Highlight not available.".to_string());
    }
    
    if state.compositor.is_none() {
        return Err("Compositor global not found".to_string());
    }
    
    if state.shm.is_none() {
        return Err("SHM global not found".to_string());
    }
    
    // Find the output for the highlight position
    let (target_output, rel_x, rel_y, scale) = if let Some(output_info) = state.find_output_for_point(x, y) {
        // Convert to output-relative coordinates
        let rel_x = x - output_info.x;
        let rel_y = y - output_info.y;
        (Some(output_info.output.clone()), rel_x, rel_y, output_info.scale)
    } else {
        // Fall back to using coordinates as-is on default output
        (None, x, y, 1.0)
    };
    
    // Layer-shell uses logical (scaled) coordinates
    // Input x, y are already in logical space (Hyprland workspace coordinates)
    // Input width, height are in physical pixels, so we need to convert them to logical
    let logical_width = (width as f64 / scale).round() as i32;
    let logical_height = (height as f64 / scale).round() as i32;
    // rel_x, rel_y are already logical (input logical coords minus output's logical position)
    let logical_rel_x = rel_x;
    let logical_rel_y = rel_y;
    
    // Clone the shm reference for later use (after mutable borrows)
    let shm = state.shm.clone().unwrap();
    
    // Create surface
    let surface = state.compositor.as_ref().unwrap().create_surface(&qh, ());
    state.surface = Some(surface.clone());
    
    // Create layer surface on overlay layer (above everything)
    let layer_surface = state.layer_shell.as_ref().unwrap().get_layer_surface(
        &surface,
        target_output.as_ref(),
        zwlr_layer_shell_v1::Layer::Overlay,
        "omnirec-highlight".to_string(),
        &qh,
        (),
    );
    
    // Configure the layer surface
    // Anchor to top-left only - position via margins, size explicitly set
    layer_surface.set_anchor(
        zwlr_layer_surface_v1::Anchor::Top | zwlr_layer_surface_v1::Anchor::Left
    );
    
    // Set explicit size (in logical pixels)
    layer_surface.set_size(logical_width as u32, logical_height as u32);
    
    // Set margins to position the surface (top, right, bottom, left)
    // With top-left anchor, only top and left margins matter for positioning
    layer_surface.set_margin(logical_rel_y, 0, 0, logical_rel_x);
    
    // Don't reserve exclusive zone
    layer_surface.set_exclusive_zone(-1);
    
    // No keyboard interactivity
    layer_surface.set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);
    
    state.layer_surface = Some(layer_surface);
    
    // Commit to trigger configure event
    surface.commit();
    
    // Wait for configure event
    while !state.configured && !state.closed {
        event_queue.blocking_dispatch(&mut state)
            .map_err(|e| format!("Dispatch failed: {}", e))?;
    }
    
    if state.closed {
        return Ok(());
    }
    
    // Create and attach the buffer with border graphic
    // Buffer should be in logical size to match the layer surface
    let buffer = create_border_buffer(&shm, &qh, logical_width, logical_height)?;
    surface.attach(Some(&buffer), 0, 0);
    
    // Mark the entire surface as damaged
    surface.damage(0, 0, i32::MAX, i32::MAX);
    surface.commit();
    
    // Flush to ensure the buffer is sent
    event_queue.flush()
        .map_err(|e| format!("Flush failed: {}", e))?;
    
    // Do another roundtrip to ensure compositor processed it
    event_queue.roundtrip(&mut state)
        .map_err(|e| format!("Roundtrip after commit failed: {}", e))?;
    
    // Run event loop for the highlight duration
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(HIGHLIGHT_DURATION_MS) {
        // Check for cancellation
        if CANCEL_FLAG.load(Ordering::SeqCst) {
            eprintln!("[Linux Highlight] Cancelled");
            break;
        }
        
        // Non-blocking dispatch with timeout
        event_queue.flush()
            .map_err(|e| format!("Flush failed: {}", e))?;
        
        // Read events with a short timeout
        if let Some(guard) = conn.prepare_read() {
            let _ = guard.read();
        }
        
        event_queue.dispatch_pending(&mut state)
            .map_err(|e| format!("Dispatch failed: {}", e))?;
        
        if state.closed {
            break;
        }
        
        thread::sleep(Duration::from_millis(16));
    }
    
    // Cleanup - destroy layer surface and surface
    if let Some(ls) = state.layer_surface.take() {
        ls.destroy();
    }
    if let Some(s) = state.surface.take() {
        s.destroy();
    }
    
    // Final flush
    let _ = event_queue.flush();
    
    Ok(())
}

/// Create an SHM buffer with the border graphic
fn create_border_buffer(
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<HighlightState>,
    width: i32,
    height: i32,
) -> Result<wl_buffer::WlBuffer, String> {
    let stride = width * 4; // 4 bytes per pixel (ARGB8888)
    let size = (stride * height) as usize;
    
    // Create a temporary file for the shared memory
    let fd = create_shm_fd(size)?;
    
    // Memory map the file and draw the border
    unsafe {
        let ptr = libc::mmap(
            std::ptr::null_mut(),
            size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd.as_fd().as_raw_fd(),
            0,
        );
        
        if ptr == libc::MAP_FAILED {
            return Err("mmap failed".to_string());
        }
        
        // Draw the border
        let pixels = ptr as *mut u8;
        draw_border(pixels, width, height, stride);
        
        // Unmap - the fd keeps the data
        libc::munmap(ptr, size);
    }
    
    // Create SHM pool and buffer
    let pool = shm.create_pool(fd.as_fd(), size as i32, qh, ());
    let buffer = pool.create_buffer(
        0,
        width,
        height,
        stride,
        wl_shm::Format::Argb8888,
        qh,
        (),
    );
    
    // We can destroy the pool immediately - buffer keeps reference
    pool.destroy();
    
    Ok(buffer)
}

/// Create a file descriptor for shared memory
fn create_shm_fd(size: usize) -> Result<OwnedFd, String> {
    use std::os::fd::FromRawFd;
    
    // Use memfd_create for anonymous shared memory
    let name = std::ffi::CString::new("omnirec-highlight").unwrap();
    let fd = unsafe {
        libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC)
    };
    
    if fd < 0 {
        return Err("memfd_create failed".to_string());
    }
    
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    
    // Set the size
    if unsafe { libc::ftruncate(fd.as_fd().as_raw_fd(), size as i64) } < 0 {
        return Err("ftruncate failed".to_string());
    }
    
    Ok(fd)
}

/// Draw the border graphic to the buffer
unsafe fn draw_border(pixels: *mut u8, width: i32, height: i32, stride: i32) {
    // ARGB8888 format: bytes are [B, G, R, A] in memory (little-endian)
    for py in 0..height {
        for px in 0..width {
            let idx = (py * stride + px * 4) as usize;
            
            let is_border = py < BORDER_WIDTH
                || py >= height - BORDER_WIDTH
                || px < BORDER_WIDTH
                || px >= width - BORDER_WIDTH;
            
            if is_border {
                // Blue border with full alpha
                *pixels.add(idx) = BORDER_B;     // Blue
                *pixels.add(idx + 1) = BORDER_G; // Green
                *pixels.add(idx + 2) = BORDER_R; // Red
                *pixels.add(idx + 3) = BORDER_A; // Alpha
            } else {
                // Transparent interior (alpha = 0)
                *pixels.add(idx) = 0;
                *pixels.add(idx + 1) = 0;
                *pixels.add(idx + 2) = 0;
                *pixels.add(idx + 3) = 0;
            }
        }
    }
}

// Wayland dispatch implementations

impl Dispatch<wl_registry::WlRegistry, ()> for HighlightState {
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
                "wl_compositor" => {
                    let compositor = registry.bind::<wl_compositor::WlCompositor, _, _>(
                        name,
                        version.min(4),
                        qh,
                        (),
                    );
                    state.compositor = Some(compositor);
                }
                "wl_shm" => {
                    let shm = registry.bind::<wl_shm::WlShm, _, _>(
                        name,
                        version.min(1),
                        qh,
                        (),
                    );
                    state.shm = Some(shm);
                }
                "zwlr_layer_shell_v1" => {
                    let layer_shell = registry.bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(
                        name,
                        version.min(4),
                        qh,
                        (),
                    );
                    state.layer_shell = Some(layer_shell);
                }
                "wl_output" => {
                    let output = registry.bind::<wl_output::WlOutput, _, _>(
                        name,
                        version.min(4),
                        qh,
                        name, // Pass the registry name as user data to identify this output
                    );
                    state.outputs.push(OutputInfo {
                        output,
                        name: String::new(), // Will be filled in by Name event
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                        scale: 1.0,
                        done: false,
                    });
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_output::WlOutput, u32> for HighlightState {
    fn event(
        state: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        _data: &u32,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Find this output in our list
        let output_info = state.outputs.iter_mut().find(|o| o.output.id() == output.id());
        
        if let Some(info) = output_info {
            match event {
                wl_output::Event::Geometry { x, y, .. } => {
                    info.x = x;
                    info.y = y;
                }
                wl_output::Event::Mode { width, height, flags, .. } => {
                    // Only use the current mode
                    use wayland_client::WEnum;
                    if let WEnum::Value(mode_flags) = flags {
                        if mode_flags.contains(wl_output::Mode::Current) {
                            info.width = width;
                            info.height = height;
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

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for HighlightState {
    fn event(
        state: &mut Self,
        layer_surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, .. } => {
                layer_surface.ack_configure(serial);
                state.configured = true;
            }
            zwlr_layer_surface_v1::Event::Closed => {
                state.closed = true;
            }
            _ => {}
        }
    }
}

// No-op dispatchers for objects we don't need events from
delegate_noop!(HighlightState: ignore wl_compositor::WlCompositor);
delegate_noop!(HighlightState: ignore wl_shm::WlShm);
delegate_noop!(HighlightState: ignore wl_shm_pool::WlShmPool);
delegate_noop!(HighlightState: ignore wl_buffer::WlBuffer);
delegate_noop!(HighlightState: ignore wl_surface::WlSurface);
delegate_noop!(HighlightState: ignore zwlr_layer_shell_v1::ZwlrLayerShellV1);

use std::os::fd::AsRawFd;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_highlight_does_not_panic() {
        // Just verify it doesn't crash (actual display requires Wayland)
        // This will fail gracefully if not running on Wayland
        show_highlight(100, 100, 800, 600);
        thread::sleep(Duration::from_millis(100));
    }
}
