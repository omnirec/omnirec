//! PipeWire video capture for Linux/Wayland.
//!
//! This module handles receiving video frames from a PipeWire stream
//! after the portal has granted access.
//!
//! # Window Resize Handling
//! When capturing a window that gets resized, PipeWire will renegotiate
//! the stream format. The `param_changed` callback updates the dimensions
//! and the new frame sizes are automatically handled.
//!
//! # Window Close Handling  
//! When the captured window is closed, the PipeWire stream transitions
//! to an error state. This triggers the stop flag and cleanly exits capture.

use crate::capture::types::{CapturedFrame, FrameReceiver, StopHandle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

use pipewire as pw;
use pw::spa;
use spa::pod::Pod;

/// Region specification for cropping frames.
#[derive(Debug, Clone, Copy)]
pub struct CropRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Start capturing from a PipeWire stream.
///
/// # Arguments
/// * `node_id` - The PipeWire node ID returned by the portal
/// * `width` - Expected frame width
/// * `height` - Expected frame height
///
/// # Returns
/// A tuple of (frame_receiver, stop_handle) for receiving frames and stopping capture.
pub fn start_pipewire_capture(
    node_id: u32,
    width: u32,
    height: u32,
) -> Result<(FrameReceiver, StopHandle), String> {
    start_pipewire_capture_internal(node_id, width, height, None, false)
}

/// Start capturing with auto-crop detection.
/// This will analyze the first frame to find the actual content bounds
/// and crop to that region (useful for window captures on GNOME).
pub fn start_pipewire_capture_with_auto_crop(
    node_id: u32,
    width: u32,
    height: u32,
) -> Result<(FrameReceiver, StopHandle), String> {
    start_pipewire_capture_internal(node_id, width, height, None, true)
}

/// Start capturing from a PipeWire stream with optional cropping.
///
/// # Arguments
/// * `node_id` - The PipeWire node ID returned by the portal
/// * `width` - Expected frame width (full stream width if cropping)
/// * `height` - Expected frame height (full stream height if cropping)
/// * `crop_region` - Optional region to crop from the stream
///
/// # Returns
/// A tuple of (frame_receiver, stop_handle) for receiving frames and stopping capture.
pub fn start_pipewire_capture_with_crop(
    node_id: u32,
    width: u32,
    height: u32,
    crop_region: Option<CropRegion>,
) -> Result<(FrameReceiver, StopHandle), String> {
    start_pipewire_capture_internal(node_id, width, height, crop_region, false)
}

/// Internal capture function with all options.
fn start_pipewire_capture_internal(
    node_id: u32,
    width: u32,
    height: u32,
    crop_region: Option<CropRegion>,
    enable_auto_crop: bool,
) -> Result<(FrameReceiver, StopHandle), String> {
    let (frame_tx, frame_rx) = mpsc::channel::<CapturedFrame>(2);
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();

    if let Some(crop) = crop_region {
        eprintln!("[PipeWire] Starting capture thread for node {} ({}x{}) with crop region ({}x{} at {},{}", 
            node_id, width, height, crop.width, crop.height, crop.x, crop.y);
    } else if enable_auto_crop {
        eprintln!("[PipeWire] Starting capture thread for node {} ({}x{}) with auto-crop enabled", node_id, width, height);
    } else {
        eprintln!("[PipeWire] Starting capture thread for node {} ({}x{})", node_id, width, height);
    }

    // Spawn the PipeWire capture thread
    std::thread::spawn(move || {
        if let Err(e) = run_pipewire_capture(node_id, width, height, crop_region, enable_auto_crop, frame_tx, stop_flag_clone) {
            eprintln!("[PipeWire] Capture error: {}", e);
        }
        eprintln!("[PipeWire] Capture thread exited");
    });

    Ok((frame_rx, stop_flag))
}

/// Data shared with stream callbacks.
struct StreamData {
    width: u32,
    height: u32,
    format: spa::param::video::VideoInfoRaw,
    frame_tx: mpsc::Sender<CapturedFrame>,
    stop_flag: Arc<AtomicBool>,
    /// Track if we've received any frames (for debugging)
    frames_received: u64,
    /// Track format changes (indicates window resize)
    format_changes: u32,
    /// Optional region to crop from the stream
    crop_region: Option<CropRegion>,
    /// Auto-detected content bounds (for window captures without explicit crop)
    auto_crop: Option<CropRegion>,
    /// Whether we should try to auto-detect content bounds
    enable_auto_crop: bool,
}

/// Run the PipeWire main loop and capture frames.
fn run_pipewire_capture(
    node_id: u32,
    width: u32,
    height: u32,
    crop_region: Option<CropRegion>,
    enable_auto_crop: bool,
    frame_tx: mpsc::Sender<CapturedFrame>,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    eprintln!("[PipeWire] Initializing PipeWire...");
    
    pw::init();

    let mainloop = pw::main_loop::MainLoop::new(None)
        .map_err(|e| format!("Failed to create main loop: {}", e))?;
    
    let context = pw::context::Context::new(&mainloop)
        .map_err(|e| format!("Failed to create context: {}", e))?;
    
    let core = context
        .connect(None)
        .map_err(|e| format!("Failed to connect to PipeWire daemon: {}", e))?;

    eprintln!("[PipeWire] Connected to PipeWire daemon");

    // Create stream
    let stream = pw::stream::Stream::new(
        &core,
        "omnirec",
        pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        },
    )
    .map_err(|e| format!("Failed to create stream: {}", e))?;

    // Shared state for the stream listener
    let stream_data = StreamData {
        width,
        height,
        format: Default::default(),
        frame_tx,
        stop_flag: stop_flag.clone(),
        frames_received: 0,
        format_changes: 0,
        crop_region,
        auto_crop: None,
        enable_auto_crop,
    };

    // Clone mainloop for stop check
    let mainloop_weak = mainloop.downgrade();
    let stop_flag_for_state = stop_flag.clone();

    // Register stream events
    let _listener = stream
        .add_local_listener_with_user_data(stream_data)
        .state_changed(move |_, _, old, new| {
            eprintln!("[PipeWire] Stream state: {:?} -> {:?}", old, new);
            
            match new {
                pw::stream::StreamState::Error(msg) => {
                    // Stream error - likely window closed or capture target unavailable
                    eprintln!("[PipeWire] Stream error (target may have closed): {}", msg);
                    stop_flag_for_state.store(true, Ordering::SeqCst);
                    if let Some(mainloop) = mainloop_weak.upgrade() {
                        mainloop.quit();
                    }
                }
                pw::stream::StreamState::Unconnected => {
                    // Stream disconnected - capture source gone
                    eprintln!("[PipeWire] Stream disconnected - stopping capture");
                    stop_flag_for_state.store(true, Ordering::SeqCst);
                    if let Some(mainloop) = mainloop_weak.upgrade() {
                        mainloop.quit();
                    }
                }
                pw::stream::StreamState::Streaming => {
                    eprintln!("[PipeWire] Stream is now streaming");
                }
                pw::stream::StreamState::Paused => {
                    // Only stop if we were previously streaming.
                    // Normal startup goes: Unconnected -> Connecting -> Paused -> Streaming
                    // User-initiated pause goes: Streaming -> Paused
                    if matches!(old, pw::stream::StreamState::Streaming) {
                        eprintln!("[PipeWire] Stream paused after streaming - stopping capture");
                        stop_flag_for_state.store(true, Ordering::SeqCst);
                        if let Some(mainloop) = mainloop_weak.upgrade() {
                            mainloop.quit();
                        }
                    } else {
                        eprintln!("[PipeWire] Stream paused (startup phase, waiting for streaming)");
                    }
                }
                _ => {}
            }
        })
        .param_changed(|_, user_data, id, param| {
            let Some(param) = param else { return };
            if id != pw::spa::param::ParamType::Format.as_raw() {
                return;
            }
            
            // Parse the format
            let (media_type, media_subtype) = match pw::spa::param::format_utils::parse_format(param) {
                Ok(v) => v,
                Err(_) => return,
            };
            
            if media_type != pw::spa::param::format::MediaType::Video
                || media_subtype != pw::spa::param::format::MediaSubtype::Raw
            {
                return;
            }
            
            // Store old dimensions for comparison
            let old_width = user_data.width;
            let old_height = user_data.height;
            
            // Parse video format info
            if let Err(e) = user_data.format.parse(param) {
                eprintln!("[PipeWire] Failed to parse video format: {:?}", e);
                return;
            }
            
            // Update dimensions from negotiated format
            user_data.width = user_data.format.size().width;
            user_data.height = user_data.format.size().height;
            user_data.format_changes += 1;
            
            // Log format info
            if user_data.format_changes == 1 {
                eprintln!("[PipeWire] Initial video format:");
            } else {
                eprintln!("[PipeWire] Format renegotiated (window resize detected):");
                eprintln!("  old size: {}x{}", old_width, old_height);
            }
            eprintln!("  format: {:?}", user_data.format.format());
            eprintln!("  size: {}x{}", user_data.width, user_data.height);
            eprintln!("  framerate: {}/{}", user_data.format.framerate().num, user_data.format.framerate().denom);
        })
        .process(|stream, user_data| {
            if user_data.stop_flag.load(Ordering::Relaxed) {
                // Only log once when stop flag is first detected
                static LOGGED_STOP: AtomicBool = AtomicBool::new(false);
                if !LOGGED_STOP.swap(true, Ordering::Relaxed) {
                    eprintln!("[PipeWire] process: stop flag is set, skipping remaining frames");
                }
                return;
            }

            match stream.dequeue_buffer() {
                Some(mut buffer) => {
                    user_data.frames_received += 1;
                    // Log periodically instead of every frame
                    if user_data.frames_received == 1 || user_data.frames_received % 100 == 0 {
                        eprintln!("[PipeWire] Processing frame #{}", user_data.frames_received);
                    }
                    process_buffer(&mut buffer, user_data);
                }
                None => {
                    // This is normal when the producer hasn't provided a new buffer yet
                }
            }
        })
        .register()
        .map_err(|e| format!("Failed to register stream listener: {}", e))?;

    // Request video format with preference for formats we can handle
    // This helps negotiate a linear buffer format instead of tiled DMA-BUF
    let obj = pw::spa::pod::object!(
        pw::spa::utils::SpaTypes::ObjectParamFormat,
        pw::spa::param::ParamType::EnumFormat,
        pw::spa::pod::property!(
            pw::spa::param::format::FormatProperties::MediaType,
            Id,
            pw::spa::param::format::MediaType::Video
        ),
        pw::spa::pod::property!(
            pw::spa::param::format::FormatProperties::MediaSubtype,
            Id,
            pw::spa::param::format::MediaSubtype::Raw
        ),
        pw::spa::pod::property!(
            pw::spa::param::format::FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            pw::spa::param::video::VideoFormat::BGRx,
            pw::spa::param::video::VideoFormat::BGRx,
            pw::spa::param::video::VideoFormat::BGRA,
            pw::spa::param::video::VideoFormat::RGBx,
            pw::spa::param::video::VideoFormat::RGBA,
            pw::spa::param::video::VideoFormat::xBGR,
            pw::spa::param::video::VideoFormat::ABGR,
            pw::spa::param::video::VideoFormat::xRGB,
            pw::spa::param::video::VideoFormat::ARGB
        ),
        pw::spa::pod::property!(
            pw::spa::param::format::FormatProperties::VideoSize,
            Choice,
            Range,
            Rectangle,
            pw::spa::utils::Rectangle { width, height },
            pw::spa::utils::Rectangle { width: 1, height: 1 },
            pw::spa::utils::Rectangle { width: 8192, height: 8192 }
        ),
        pw::spa::pod::property!(
            pw::spa::param::format::FormatProperties::VideoFramerate,
            Choice,
            Range,
            Fraction,
            pw::spa::utils::Fraction { num: 30, denom: 1 },
            pw::spa::utils::Fraction { num: 0, denom: 1 },
            pw::spa::utils::Fraction { num: 120, denom: 1 }
        ),
    );
    
    let values: Vec<u8> = pw::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pw::spa::pod::Value::Object(obj),
    )
    .map_err(|e| format!("Failed to serialize format params: {:?}", e))?
    .0
    .into_inner();
    
    let mut params = [Pod::from_bytes(&values).ok_or("Failed to create Pod from bytes")?];

    // Connect stream to the specified node
    stream
        .connect(
            spa::utils::Direction::Input,
            Some(node_id),
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        )
        .map_err(|e| format!("Failed to connect stream to node {}: {}", node_id, e))?;

    eprintln!("[PipeWire] Stream connected to node {}", node_id);
    
    // Activate the stream to start receiving buffers
    stream
        .set_active(true)
        .map_err(|e| format!("Failed to activate stream: {}", e))?;
    
    eprintln!("[PipeWire] Stream activated");

    // Set up a timer to check stop flag
    let mainloop_clone = mainloop.clone();
    let stop_flag_check = stop_flag.clone();
    
    let timer = mainloop.loop_().add_timer(move |_timer_expired_count| {
        if stop_flag_check.load(Ordering::Relaxed) {
            eprintln!("[PipeWire] Stop flag detected, quitting main loop");
            mainloop_clone.quit();
        }
    });

    timer.update_timer(
        Some(std::time::Duration::from_millis(100)),
        Some(std::time::Duration::from_millis(100)),
    );

    eprintln!("[PipeWire] Entering main loop (stop_flag={})", stop_flag.load(Ordering::Relaxed));
    
    // Keep listener and timer alive by moving them into a scope that lasts until mainloop exits
    // The mainloop.run() is blocking, so these won't be dropped until we return
    let _keep_alive = (_listener, timer);
    
    mainloop.run();
    eprintln!("[PipeWire] Main loop exited (stop_flag={})", stop_flag.load(Ordering::Relaxed));

    Ok(())
}

/// Process a buffer from the PipeWire stream.
fn process_buffer(buffer: &mut pw::buffer::Buffer, user_data: &mut StreamData) {
    let datas = buffer.datas_mut();
    if datas.is_empty() {
        return;
    }

    let data = &mut datas[0];
    
    // Get chunk info first
    let chunk = data.chunk();
    let stride = chunk.stride() as usize;
    let offset = chunk.offset() as usize;

    // Log buffer details only on first frame
    static LOGGED_BUFFER_INFO: AtomicBool = AtomicBool::new(false);
    if !LOGGED_BUFFER_INFO.swap(true, Ordering::Relaxed) {
        let data_type = data.type_();
        let chunk_size = chunk.size() as usize;
        eprintln!("[PipeWire] First buffer info:");
        eprintln!("  type: {:?}", data_type);
        eprintln!("  stride={}, size={}, offset={}", stride, chunk_size, offset);
        eprintln!("  fd={:?}, maxsize={}", data.as_raw().fd, data.as_raw().maxsize);
    }

    if stride == 0 {
        eprintln!("[PipeWire] process_buffer: invalid stride");
        return;
    }
    
    let width = user_data.width;
    let height = user_data.height;
    let bytes_per_pixel = 4; // BGRx/BGRA
    let expected_size = (height as usize) * stride;

    // Try to get the data slice
    let slice = match data.data() {
        Some(s) => s,
        None => {
            // For DmaBuf/MemFd, we may need to mmap the fd
            // Log only once
            static LOGGED_DMABUF: AtomicBool = AtomicBool::new(false);
            if !LOGGED_DMABUF.swap(true, Ordering::Relaxed) {
                eprintln!("[PipeWire] Using DMA-BUF path (mmap)");
            }
            
            // Try to access via fd for DmaBuf
            let raw = data.as_raw();
            if raw.fd != -1 {
                // Try to mmap the buffer
                unsafe {
                    let map_size = if raw.maxsize > 0 { raw.maxsize as usize } else { expected_size };
                    let ptr = libc::mmap(
                        std::ptr::null_mut(),
                        map_size,
                        libc::PROT_READ,
                        libc::MAP_SHARED,
                        raw.fd as i32,
                        raw.mapoffset as i64,
                    );
                    
                    if ptr == libc::MAP_FAILED {
                        eprintln!("[PipeWire] mmap failed: {}", std::io::Error::last_os_error());
                        return;
                    }
                    
                    // Create a slice from the mapped memory
                    let mapped_slice = std::slice::from_raw_parts(
                        (ptr as *const u8).add(offset),
                        expected_size.min(map_size.saturating_sub(offset)),
                    );
                    
                    // Process the frame
                    let frame_data = extract_frame_data(mapped_slice, width, height, stride, bytes_per_pixel);
                    
                    // Unmap
                    libc::munmap(ptr, map_size);
                    
                    if let Some(frame_data) = frame_data {
                        // Apply cropping if specified
                        if let Some(crop) = user_data.crop_region {
                            if let Some(cropped_data) = crop_frame_data(&frame_data, width, height, crop) {
                                send_frame(user_data, crop.width, crop.height, cropped_data);
                            }
                        } else {
                            send_frame(user_data, width, height, frame_data);
                        }
                    }
                    return;
                }
            }
            
            eprintln!("[PipeWire] Cannot access buffer data (no fd available)");
            return;
        }
    };

    if let Some(frame_data) = extract_frame_data(slice, width, height, stride, bytes_per_pixel) {
        // Apply cropping if specified
        if let Some(crop) = user_data.crop_region {
            if let Some(cropped_data) = crop_frame_data(&frame_data, width, height, crop) {
                send_frame(user_data, crop.width, crop.height, cropped_data);
            }
        } else {
            send_frame(user_data, width, height, frame_data);
        }
    }
}

/// Extract frame data from a buffer slice, handling stride.
fn extract_frame_data(slice: &[u8], width: u32, height: u32, stride: usize, bytes_per_pixel: usize) -> Option<Vec<u8>> {
    let row_bytes = width as usize * bytes_per_pixel;
    
    // Log stride info once
    static LOGGED_EXTRACTION: AtomicBool = AtomicBool::new(false);
    if !LOGGED_EXTRACTION.swap(true, Ordering::Relaxed) {
        eprintln!("[PipeWire] Frame extraction setup:");
        eprintln!("  dimensions: {}x{}, stride={}, row_bytes={}", width, height, stride, row_bytes);
        if stride == row_bytes {
            eprintln!("  using direct copy (stride matches)");
        } else {
            eprintln!("  using row-by-row copy (stride padding: {} bytes)", stride - row_bytes);
        }
    }
    
    // If stride matches row_bytes exactly, we can do a direct copy
    if stride == row_bytes {
        let total_bytes = (height as usize) * row_bytes;
        if slice.len() >= total_bytes {
            return Some(slice[..total_bytes].to_vec());
        }
    }
    
    // Otherwise, copy row by row to handle stride padding
    let mut frame_data = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height as usize {
        let row_start = y * stride;
        let row_end = row_start + row_bytes;
        if row_end <= slice.len() {
            frame_data.extend_from_slice(&slice[row_start..row_end]);
        } else {
            // Log only once per session
            static LOGGED_TOO_SMALL: AtomicBool = AtomicBool::new(false);
            if !LOGGED_TOO_SMALL.swap(true, Ordering::Relaxed) {
                eprintln!("[PipeWire] Warning: buffer too small at row {}: need {} but have {}", y, row_end, slice.len());
            }
            return None;
        }
    }

    Some(frame_data)
}

/// Crop frame data to a specific region.
fn crop_frame_data(frame_data: &[u8], full_width: u32, full_height: u32, crop: CropRegion) -> Option<Vec<u8>> {
    let bytes_per_pixel = 4; // BGRA
    
    // Validate crop region
    if crop.x < 0 || crop.y < 0 {
        eprintln!("[PipeWire] Invalid crop region: negative coordinates ({}, {})", crop.x, crop.y);
        return None;
    }
    
    let crop_x = crop.x as u32;
    let crop_y = crop.y as u32;
    
    // Clamp crop region to frame boundaries
    let crop_x_end = (crop_x + crop.width).min(full_width);
    let crop_y_end = (crop_y + crop.height).min(full_height);
    
    if crop_x >= full_width || crop_y >= full_height {
        eprintln!("[PipeWire] Crop region outside frame bounds");
        return None;
    }
    
    let actual_crop_width = crop_x_end - crop_x;
    let actual_crop_height = crop_y_end - crop_y;
    
    // Log if we had to clamp the region
    static LOGGED_CLAMPING: AtomicBool = AtomicBool::new(false);
    if !LOGGED_CLAMPING.swap(true, Ordering::Relaxed)
        && (actual_crop_width != crop.width || actual_crop_height != crop.height)
    {
        eprintln!("[PipeWire] Warning: crop region clamped from {}x{} to {}x{}", 
            crop.width, crop.height, actual_crop_width, actual_crop_height);
    }
    
    let mut cropped = Vec::with_capacity((actual_crop_width * actual_crop_height * bytes_per_pixel) as usize);
    
    // Copy row by row
    for y in crop_y..crop_y_end {
        let row_start = ((y * full_width + crop_x) as usize) * (bytes_per_pixel as usize);
        let row_end = row_start + ((actual_crop_width as usize) * (bytes_per_pixel as usize));
        
        if row_end <= frame_data.len() {
            cropped.extend_from_slice(&frame_data[row_start..row_end]);
        } else {
            eprintln!("[PipeWire] Crop overflow at row {}: need {} but have {}", y, row_end, frame_data.len());
            return None;
        }
    }
    
    Some(cropped)
}

/// Convert frame data to BGRA format based on the source format.
fn convert_to_bgra(frame_data: Vec<u8>, format: spa::param::video::VideoFormat) -> Vec<u8> {
    use spa::param::video::VideoFormat;
    
    match format {
        // Already BGRA or BGRx - no conversion needed
        VideoFormat::BGRA | VideoFormat::BGRx => frame_data,
        
        // RGBA/RGBx -> BGRA: swap R and B
        VideoFormat::RGBA | VideoFormat::RGBx => {
            let mut converted = frame_data;
            for chunk in converted.chunks_exact_mut(4) {
                chunk.swap(0, 2); // Swap R and B
            }
            converted
        }
        
        // ARGB -> BGRA: shift ARGB to BGRA (A,R,G,B -> B,G,R,A)
        VideoFormat::ARGB => {
            let mut converted = Vec::with_capacity(frame_data.len());
            for chunk in frame_data.chunks_exact(4) {
                converted.push(chunk[3]); // B
                converted.push(chunk[2]); // G
                converted.push(chunk[1]); // R
                converted.push(chunk[0]); // A
            }
            converted
        }
        
        // xRGB -> BGRA: shift xRGB to BGRA (x,R,G,B -> B,G,R,255)
        VideoFormat::xRGB => {
            let mut converted = Vec::with_capacity(frame_data.len());
            for chunk in frame_data.chunks_exact(4) {
                converted.push(chunk[3]); // B
                converted.push(chunk[2]); // G
                converted.push(chunk[1]); // R
                converted.push(255);      // A (fully opaque)
            }
            converted
        }
        
        // ABGR -> BGRA: A,B,G,R -> B,G,R,A
        VideoFormat::ABGR => {
            let mut converted = Vec::with_capacity(frame_data.len());
            for chunk in frame_data.chunks_exact(4) {
                converted.push(chunk[1]); // B
                converted.push(chunk[2]); // G
                converted.push(chunk[3]); // R
                converted.push(chunk[0]); // A
            }
            converted
        }
        
        // xBGR -> BGRA: x,B,G,R -> B,G,R,255
        VideoFormat::xBGR => {
            let mut converted = Vec::with_capacity(frame_data.len());
            for chunk in frame_data.chunks_exact(4) {
                converted.push(chunk[1]); // B
                converted.push(chunk[2]); // G
                converted.push(chunk[3]); // R
                converted.push(255);      // A (fully opaque)
            }
            converted
        }
        
        // Unknown format - log warning and return as-is
        _ => {
            static LOGGED_UNKNOWN: AtomicBool = AtomicBool::new(false);
            if !LOGGED_UNKNOWN.swap(true, Ordering::Relaxed) {
                eprintln!("[PipeWire] Warning: unknown video format {:?}, colors may be incorrect", format);
            }
            frame_data
        }
    }
}

/// Detect the bounding box of non-black content in a frame.
/// Returns (x, y, width, height) of the content region, or None if detection fails.
fn detect_content_bounds(frame_data: &[u8], width: u32, height: u32) -> Option<CropRegion> {
    let bytes_per_pixel = 4;
    let row_bytes = width as usize * bytes_per_pixel;
    
    // Threshold for considering a pixel as "content" (not black)
    // Using a small threshold to account for compression artifacts
    const THRESHOLD: u8 = 8;
    
    let mut min_x = width as i32;
    let mut min_y = height as i32;
    let mut max_x = 0i32;
    let mut max_y = 0i32;
    
    for y in 0..height as usize {
        for x in 0..width as usize {
            let idx = y * row_bytes + x * bytes_per_pixel;
            if idx + 2 < frame_data.len() {
                let b = frame_data[idx];
                let g = frame_data[idx + 1];
                let r = frame_data[idx + 2];
                
                // Check if pixel is non-black
                if r > THRESHOLD || g > THRESHOLD || b > THRESHOLD {
                    min_x = min_x.min(x as i32);
                    min_y = min_y.min(y as i32);
                    max_x = max_x.max(x as i32);
                    max_y = max_y.max(y as i32);
                }
            }
        }
    }
    
    // Check if we found any content
    if max_x >= min_x && max_y >= min_y {
        let content_width = (max_x - min_x + 1) as u32;
        let content_height = (max_y - min_y + 1) as u32;
        
        // Only use auto-crop if content is meaningfully smaller than frame
        // (at least 10% smaller in either dimension)
        let width_ratio = content_width as f32 / width as f32;
        let height_ratio = content_height as f32 / height as f32;
        
        if width_ratio < 0.95 || height_ratio < 0.95 {
            eprintln!("[PipeWire] Auto-crop detected content bounds: {}x{} at ({}, {})",
                content_width, content_height, min_x, min_y);
            return Some(CropRegion {
                x: min_x,
                y: min_y,
                width: content_width,
                height: content_height,
            });
        }
    }
    
    None
}

/// Send a frame to the encoder channel.
fn send_frame(user_data: &mut StreamData, width: u32, height: u32, frame_data: Vec<u8>) {
    // Convert to BGRA format for consistent downstream processing
    let format = user_data.format.format();
    let bgra_data = convert_to_bgra(frame_data, format);
    
    // Auto-crop detection on first frame
    if user_data.enable_auto_crop && user_data.auto_crop.is_none() && user_data.frames_received == 1 {
        if let Some(bounds) = detect_content_bounds(&bgra_data, width, height) {
            user_data.auto_crop = Some(bounds);
        } else {
            // Disable auto-crop if we couldn't detect bounds
            eprintln!("[PipeWire] Auto-crop: no distinct content bounds detected, using full frame");
            user_data.enable_auto_crop = false;
        }
    }
    
    // Apply auto-crop if detected
    let (final_data, final_width, final_height) = if let Some(crop) = user_data.auto_crop {
        if let Some(cropped) = crop_frame_data(&bgra_data, width, height, crop) {
            (cropped, crop.width, crop.height)
        } else {
            (bgra_data, width, height)
        }
    } else {
        (bgra_data, width, height)
    };
    
    let frame = CapturedFrame {
        width: final_width,
        height: final_height,
        data: final_data,
    };
    
    // Non-blocking send - drop frame if channel is full
    match user_data.frame_tx.try_send(frame) {
        Ok(()) => {
            // Success - no logging needed for every frame
        }
        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
            // Log dropped frames periodically
            static DROPS: AtomicBool = AtomicBool::new(false);
            if !DROPS.swap(true, Ordering::Relaxed) {
                eprintln!("[PipeWire] Warning: encoder falling behind, dropping frames");
            }
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
            eprintln!("[PipeWire] Frame channel closed, stopping capture");
            user_data.stop_flag.store(true, Ordering::SeqCst);
        }
    }
}
