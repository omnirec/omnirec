//! Region recording using Windows.Graphics.Capture API for monitor capture with cropping.

use crate::capture::types::CapturedFrame;
use crate::capture::CaptureRegion;
use crate::capture::windows::monitor_list;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use windows_capture::{
    capture::{Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
};

/// Flags passed to the region capture handler.
pub struct RegionCaptureFlags {
    pub frame_tx: mpsc::Sender<CapturedFrame>,
    pub stop_flag: Arc<AtomicBool>,
    pub region: CaptureRegion,
}

/// Frame capture handler for monitor-based region capture.
struct RegionCaptureHandler {
    frame_tx: mpsc::Sender<CapturedFrame>,
    stop_flag: Arc<AtomicBool>,
    region: CaptureRegion,
    #[allow(dead_code)]
    frame_count: u64,
    #[allow(dead_code)]
    dropped_count: u64,
}

impl GraphicsCaptureApiHandler for RegionCaptureHandler {
    type Flags = RegionCaptureFlags;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            frame_tx: ctx.flags.frame_tx,
            stop_flag: ctx.flags.stop_flag,
            region: ctx.flags.region,
            frame_count: 0,
            dropped_count: 0,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        // Check if we should stop
        if self.stop_flag.load(Ordering::Relaxed) {
            capture_control.stop();
            return Ok(());
        }

        // Get frame buffer
        let mut buffer = frame.buffer()?;
        let full_width = buffer.width();
        let full_height = buffer.height();
        let raw_data = buffer.as_raw_buffer();

        // Calculate stride (bytes per row in the buffer)
        let buffer_stride = raw_data.len() / full_height as usize;

        // Region coordinates from the frontend are already in physical pixels
        // (matching the frame buffer coordinates). No conversion needed.
        let region_x = self.region.x.max(0) as u32;
        let region_y = self.region.y.max(0) as u32;
        let region_width = self.region.width;
        let region_height = self.region.height;

        // Debug: log frame dimensions on first frame
        if self.frame_count == 0 {
            eprintln!("[Windows] First frame received:");
            eprintln!("[Windows]   Frame dimensions: {}x{}", full_width, full_height);
            eprintln!("[Windows]   Buffer stride: {} bytes/row", buffer_stride);
            eprintln!("[Windows]   Region (physical): x={}, y={}, {}x{}", 
                region_x, region_y, region_width, region_height);
        }

        // Clamp to frame bounds
        let region_x = region_x.min(full_width);
        let region_y = region_y.min(full_height);
        let region_width = region_width.min(full_width.saturating_sub(region_x));
        let region_height = region_height.min(full_height.saturating_sub(region_y));

        if region_width == 0 || region_height == 0 {
            // Skip invalid frames
            return Ok(());
        }

        // Crop the frame to the region
        let cropped_data = crop_frame(
            raw_data,
            full_width,
            buffer_stride,
            region_x,
            region_y,
            region_width,
            region_height,
        );

        let captured_frame = CapturedFrame {
            width: region_width,
            height: region_height,
            data: cropped_data,
        };

        // Try to send frame
        match self.frame_tx.try_send(captured_frame) {
            Ok(()) => {
                self.frame_count += 1;
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                // Channel full, drop frame (encoder can't keep up)
                self.dropped_count += 1;
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                // Channel closed, stop capture
                capture_control.stop();
                return Ok(());
            }
        }

        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.stop_flag.store(true, Ordering::Relaxed);
        Ok(())
    }
}

/// Crop a frame buffer to the specified region.
///
/// # Arguments
/// * `data` - Source BGRA pixel data
/// * `full_width` - Full frame width in pixels
/// * `buffer_stride` - Bytes per row in the source buffer (may include padding)
/// * `x`, `y` - Region top-left position
/// * `width`, `height` - Region dimensions
fn crop_frame(
    data: &[u8],
    _full_width: u32,
    buffer_stride: usize,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let pixel_stride = 4usize; // BGRA
    let output_row_bytes = (width as usize) * pixel_stride;
    let mut output = Vec::with_capacity(output_row_bytes * height as usize);

    for row in 0..height {
        let src_y = (y + row) as usize;
        let src_row_start = src_y * buffer_stride;
        let src_x_offset = (x as usize) * pixel_stride;
        let src_start = src_row_start + src_x_offset;
        let src_end = src_start + output_row_bytes;

        if src_end <= data.len() {
            output.extend_from_slice(&data[src_start..src_end]);
        } else {
            // Fill with black if out of bounds
            output.extend(std::iter::repeat_n(0u8, output_row_bytes));
        }
    }

    output
}

/// Find a monitor by its device ID.
fn find_monitor_by_id(monitor_id: &str) -> Result<Monitor, String> {
    let monitors = Monitor::enumerate().map_err(|e| format!("Failed to enumerate monitors: {}", e))?;

    eprintln!("[Windows] Looking for monitor with id: {}", monitor_id);
    eprintln!("[Windows] Available monitors from windows-capture:");
    
    for (i, monitor) in monitors.iter().enumerate() {
        let name = monitor.device_name().unwrap_or_else(|_| "unknown".to_string());
        eprintln!("[Windows]   [{}] device_name={}", i, name);
    }

    for monitor in monitors {
        // Get device name from monitor
        if let Ok(name) = monitor.device_name() {
            if name == monitor_id {
                eprintln!("[Windows] Found matching monitor: {}", name);
                return Ok(monitor);
            }
        }
    }

    Err(format!("Monitor not found: {}", monitor_id))
}

/// Start capturing a screen region and return a receiver for cropped frames.
///
/// Returns a tuple of (frame_receiver, stop_flag).
/// Set stop_flag to true to stop capture.
pub fn start_region_capture(
    region: CaptureRegion,
) -> Result<(mpsc::Receiver<CapturedFrame>, Arc<AtomicBool>), String> {
    // Validate dimensions
    if region.width == 0 || region.height == 0 {
        return Err(format!(
            "Invalid region dimensions: {}x{}",
            region.width, region.height
        ));
    }

    // Look up monitor info for validation/debugging
    let monitors = monitor_list::list_monitors();
    let monitor_info = monitors
        .iter()
        .find(|m| m.id == region.monitor_id)
        .ok_or_else(|| format!("Monitor not found: {}", region.monitor_id))?;

    eprintln!("[Windows] === REGION CAPTURE DEBUG ===");
    eprintln!(
        "[Windows] Input region (physical coords): monitor_id={}, x={}, y={}, {}x{}",
        region.monitor_id, region.x, region.y, region.width, region.height
    );
    eprintln!(
        "[Windows] Target monitor: pos=({}, {}), size={}x{}, scale={}",
        monitor_info.x, monitor_info.y, monitor_info.width, monitor_info.height, monitor_info.scale_factor
    );
    eprintln!("[Windows] All monitors (physical coords):");
    for m in &monitors {
        eprintln!("[Windows]   {} at ({}, {}) {}x{} scale={}", m.id, m.x, m.y, m.width, m.height, m.scale_factor);
    }
    eprintln!("[Windows] =============================");

    // Find the monitor for capture
    let monitor = find_monitor_by_id(&region.monitor_id)?;

    // Create channel for frames (larger buffer for region capture which may have bursty delivery)
    let (frame_tx, frame_rx) = mpsc::channel::<CapturedFrame>(120);

    // Create stop flag
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();

    // Create flags for the handler
    let flags = RegionCaptureFlags {
        frame_tx,
        stop_flag: stop_flag_clone,
        region,
    };

    // Configure capture settings
    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::WithCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        flags,
    );

    // Start capture in a separate thread
    std::thread::spawn(move || {
        if let Err(e) = RegionCaptureHandler::start(settings) {
            eprintln!("Region capture error: {}", e);
        }
    });

    Ok((frame_rx, stop_flag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_id_matching() {
        // Compare our monitor enumeration with windows-capture's
        let our_monitors = monitor_list::list_monitors();
        let wc_monitors = Monitor::enumerate().expect("Failed to enumerate monitors");
        
        println!("\n=== MONITOR ID COMPARISON ===");
        println!("Our monitors (monitor_list):");
        for m in &our_monitors {
            println!("  id='{}', name='{}', pos=({}, {}), size={}x{}", 
                m.id, m.name, m.x, m.y, m.width, m.height);
        }
        
        println!("\nwindows-capture monitors:");
        for (i, m) in wc_monitors.iter().enumerate() {
            let name = m.device_name().unwrap_or_else(|_| "unknown".to_string());
            println!("  [{}] device_name='{}'", i, name);
        }
        
        // Verify each of our monitors can be found in windows-capture
        println!("\nMatching test:");
        for m in &our_monitors {
            let found = wc_monitors.iter().any(|wc| {
                wc.device_name().map(|n| n == m.id).unwrap_or(false)
            });
            println!("  {} -> {}", m.id, if found { "FOUND" } else { "NOT FOUND" });
            assert!(found, "Monitor {} not found in windows-capture", m.id);
        }
        println!("==============================\n");
    }

    #[test]
    fn test_crop_frame_basic() {
        // Create a 4x4 test image (each pixel is BGRA = 4 bytes)
        // Pixels are numbered 0-15 for easy tracking
        let mut data = Vec::new();
        for i in 0u8..16 {
            data.extend_from_slice(&[i, i, i, 255]); // BGRA with pixel index as color
        }

        // Crop a 2x2 region starting at (1, 1)
        let cropped = crop_frame(&data, 4, 16, 1, 1, 2, 2);

        // Expected: pixels 5,6 and 9,10
        assert_eq!(cropped.len(), 2 * 2 * 4); // 2x2 pixels, 4 bytes each

        // Check pixel values
        assert_eq!(cropped[0..4], [5, 5, 5, 255]); // Pixel (1,1) = index 5
        assert_eq!(cropped[4..8], [6, 6, 6, 255]); // Pixel (2,1) = index 6
        assert_eq!(cropped[8..12], [9, 9, 9, 255]); // Pixel (1,2) = index 9
        assert_eq!(cropped[12..16], [10, 10, 10, 255]); // Pixel (2,2) = index 10
    }
}
