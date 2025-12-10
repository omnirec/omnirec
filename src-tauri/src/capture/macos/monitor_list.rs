//! macOS monitor/display enumeration using Core Graphics.

use crate::capture::types::MonitorInfo;
use core_graphics::display::{CGDirectDisplayID, CGDisplay, CGMainDisplayID};

/// List all connected monitors on macOS.
///
/// Uses Core Graphics CGGetActiveDisplayList to enumerate displays.
pub fn list_monitors() -> Vec<MonitorInfo> {
    let mut monitors = Vec::new();

    // Get the main display ID for primary detection
    let main_display_id = unsafe { CGMainDisplayID() };

    // Get list of all active displays (max 32 displays)
    let max_displays: u32 = 32;
    let mut display_ids: Vec<CGDirectDisplayID> = vec![0; max_displays as usize];
    let mut display_count: u32 = 0;

    let result = unsafe {
        core_graphics::display::CGGetActiveDisplayList(
            max_displays,
            display_ids.as_mut_ptr(),
            &mut display_count,
        )
    };

    if result != 0 {
        eprintln!("[macOS] CGGetActiveDisplayList failed with error: {}", result);
        return monitors;
    }

    // Truncate to actual count
    display_ids.truncate(display_count as usize);

    for display_id in display_ids {
        let display = CGDisplay::new(display_id);
        let bounds = display.bounds();
        let is_primary = display_id == main_display_id;

        // Get display name - Core Graphics doesn't provide names directly,
        // so we create a descriptive name based on properties
        let name = if is_primary {
            format!("Display {} (Primary)", display_id)
        } else {
            format!("Display {}", display_id)
        };

        // Get physical pixel dimensions (for calculating scale factor)
        let physical_width = display.pixels_wide() as f64;
        
        // bounds.size gives logical dimensions
        let logical_width = bounds.size.width;
        let logical_height = bounds.size.height;
        
        // Calculate scale factor (physical pixels / logical pixels)
        // This is typically 2.0 for Retina displays, 1.0 for standard displays
        let scale_factor = if logical_width > 0.0 {
            physical_width / logical_width
        } else {
            1.0
        };

        // Monitor coordinates are in logical pixels (from Core Graphics)
        // Frontend will convert Tauri's physical coords to logical to match
        let logical_x = bounds.origin.x as i32;
        let logical_y = bounds.origin.y as i32;
        
        monitors.push(MonitorInfo {
            id: display_id.to_string(),
            name,
            x: logical_x,
            y: logical_y,
            // Return logical dimensions for UI coordinate matching
            // The capture code will scale these to physical using scale_factor
            width: logical_width as u32,
            height: logical_height as u32,
            is_primary,
            scale_factor,
        });
    }

    // Sort with primary monitor first
    monitors.sort_by(|a, b| b.is_primary.cmp(&a.is_primary));

    monitors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_monitors() {
        let monitors = list_monitors();
        // Should have at least one monitor on any macOS system
        assert!(!monitors.is_empty(), "Should have at least one monitor");

        // First monitor should be primary
        if !monitors.is_empty() {
            assert!(monitors[0].is_primary, "First monitor should be primary");
        }

        // All monitors should have valid dimensions
        for monitor in &monitors {
            assert!(monitor.width > 0, "Monitor width should be > 0");
            assert!(monitor.height > 0, "Monitor height should be > 0");
            assert!(!monitor.id.is_empty(), "Monitor ID should not be empty");
        }
    }
}
