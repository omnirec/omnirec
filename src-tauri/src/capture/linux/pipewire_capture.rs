//! PipeWire video capture for Linux/Wayland.
//!
//! This module handles receiving video frames from a PipeWire stream
//! after the portal has granted access.

use crate::capture::types::{CapturedFrame, FrameReceiver, StopHandle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

use pipewire as pw;
use pw::spa;
use spa::pod::Pod;

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
    let (frame_tx, frame_rx) = mpsc::channel::<CapturedFrame>(2);
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();

    eprintln!("[PipeWire] Starting capture thread for node {} ({}x{})", node_id, width, height);

    // Spawn the PipeWire capture thread
    std::thread::spawn(move || {
        if let Err(e) = run_pipewire_capture(node_id, width, height, frame_tx, stop_flag_clone) {
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
}

/// Run the PipeWire main loop and capture frames.
fn run_pipewire_capture(
    node_id: u32,
    width: u32,
    height: u32,
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
        "screen-recorder",
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
    };

    // Clone mainloop for stop check
    let mainloop_weak = mainloop.downgrade();
    let stop_flag_for_state = stop_flag.clone();

    // Register stream events
    let _listener = stream
        .add_local_listener_with_user_data(stream_data)
        .state_changed(move |_, _, old, new| {
            eprintln!("[PipeWire] Stream state: {:?} -> {:?}", old, new);
            // Check if we should stop on error
            if matches!(new, pw::stream::StreamState::Error(_)) {
                stop_flag_for_state.store(true, Ordering::SeqCst);
                if let Some(mainloop) = mainloop_weak.upgrade() {
                    mainloop.quit();
                }
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
            
            // Parse video format info
            if let Err(e) = user_data.format.parse(param) {
                eprintln!("[PipeWire] Failed to parse video format: {:?}", e);
                return;
            }
            
            eprintln!("[PipeWire] Negotiated video format:");
            eprintln!("  format: {:?}", user_data.format.format());
            eprintln!("  size: {}x{}", user_data.format.size().width, user_data.format.size().height);
            eprintln!("  framerate: {}/{}", user_data.format.framerate().num, user_data.format.framerate().denom);
            
            // Update dimensions from negotiated format
            user_data.width = user_data.format.size().width;
            user_data.height = user_data.format.size().height;
        })
        .process(|stream, user_data| {
            if user_data.stop_flag.load(Ordering::Relaxed) {
                eprintln!("[PipeWire] process: stop flag is set, skipping");
                return;
            }

            match stream.dequeue_buffer() {
                Some(mut buffer) => {
                    eprintln!("[PipeWire] Got buffer, processing frame");
                    process_buffer(&mut buffer, user_data);
                }
                None => {
                    eprintln!("[PipeWire] process called but no buffer available");
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
    
    let timer = mainloop.loop_().add_timer(move |timer_expired_count| {
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
        eprintln!("[PipeWire] process_buffer: no data in buffer");
        return;
    }

    let data = &mut datas[0];
    
    // Check the buffer type
    let data_type = data.type_();
    eprintln!("[PipeWire] Buffer type: {:?}", data_type);
    
    // Get chunk info first
    let chunk = data.chunk();
    let stride = chunk.stride() as usize;
    let chunk_size = chunk.size() as usize;
    let offset = chunk.offset() as usize;

    eprintln!("[PipeWire] Chunk: stride={}, size={}, offset={}", stride, chunk_size, offset);
    eprintln!("[PipeWire] Data: fd={:?}, maxsize={}, mapoffset={}", 
        data.as_raw().fd, data.as_raw().maxsize, data.as_raw().mapoffset);

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
            eprintln!("[PipeWire] No direct data access - buffer may be DMA-BUF");
            
            // Try to access via fd for DmaBuf
            let raw = data.as_raw();
            if raw.fd != -1 {
                eprintln!("[PipeWire] Has fd={}, trying mmap...", raw.fd);
                
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
                    
                    eprintln!("[PipeWire] mmap succeeded, got {} bytes", mapped_slice.len());
                    
                    // Process the frame
                    let frame_data = extract_frame_data(mapped_slice, width, height, stride, bytes_per_pixel);
                    
                    // Unmap
                    libc::munmap(ptr, map_size);
                    
                    if let Some(frame_data) = frame_data {
                        send_frame(user_data, width, height, frame_data);
                    }
                    return;
                }
            }
            
            eprintln!("[PipeWire] Cannot access buffer data");
            return;
        }
    };
    
    eprintln!("[PipeWire] Got direct slice of {} bytes", slice.len());

    if let Some(frame_data) = extract_frame_data(slice, width, height, stride, bytes_per_pixel) {
        send_frame(user_data, width, height, frame_data);
    }
}

/// Extract frame data from a buffer slice, handling stride.
fn extract_frame_data(slice: &[u8], width: u32, height: u32, stride: usize, bytes_per_pixel: usize) -> Option<Vec<u8>> {
    let row_bytes = width as usize * bytes_per_pixel;
    
    // Log stride info once
    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, Ordering::Relaxed) {
        eprintln!("[PipeWire] Frame extraction: width={}, height={}, stride={}, row_bytes={}, slice_len={}", 
            width, height, stride, row_bytes, slice.len());
        eprintln!("[PipeWire] Expected total: {} bytes, have: {} bytes", 
            height as usize * stride, slice.len());
        
        // If stride equals row_bytes, we can do a direct copy
        if stride == row_bytes {
            eprintln!("[PipeWire] Stride matches row_bytes - using direct copy");
        } else {
            eprintln!("[PipeWire] Stride differs from row_bytes - need row-by-row copy");
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
            eprintln!("[PipeWire] Buffer too small at row {}: need {} but have {}", y, row_end, slice.len());
            return None;
        }
    }

    Some(frame_data)
}

/// Send a frame to the encoder channel.
fn send_frame(user_data: &mut StreamData, width: u32, height: u32, frame_data: Vec<u8>) {
    let frame_len = frame_data.len();
    
    let frame = CapturedFrame {
        width,
        height,
        data: frame_data,
    };
    
    eprintln!("[PipeWire] Created frame {}x{} with {} bytes", width, height, frame_len);
    
    // Non-blocking send - drop frame if channel is full
    match user_data.frame_tx.try_send(frame) {
        Ok(()) => {
            eprintln!("[PipeWire] Frame sent successfully!");
        }
        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
            eprintln!("[PipeWire] Channel full, frame dropped");
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
            eprintln!("[PipeWire] Frame channel closed, stopping capture");
            user_data.stop_flag.store(true, Ordering::SeqCst);
        }
    }
}
