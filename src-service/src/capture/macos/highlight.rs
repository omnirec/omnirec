//! macOS visual highlight for capture target preview.
//!
//! Shows a temporary border around the selected capture target using NSWindow.

use core_foundation::base::TCFType;
use core_graphics::base::CGFloat;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSBackingStoreType, NSColor, NSScreen, NSView, NSWindow, NSWindowStyleMask};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize};
use std::ffi::c_void;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

const BORDER_WIDTH: CGFloat = 6.0;
const HIGHLIGHT_DURATION_MS: u64 = 800;

// Store the current highlight window pointer to manage its lifecycle safely
// We store as usize to avoid Send/Sync issues with raw pointers
// The window is only ever accessed from the main thread
static CURRENT_HIGHLIGHT: Mutex<Option<usize>> = Mutex::new(None);

/// Show a highlight border around the specified area.
///
/// The highlight is non-interactive and auto-dismisses after a short duration.
/// This function dispatches to the main thread and returns immediately.
pub fn show_highlight(x: i32, y: i32, width: i32, height: i32) {
    // Dispatch directly to main thread
    let queue = dispatch::Queue::main();
    queue.exec_async(move || {
        create_highlight_window(x, y, width, height);
    });
}

/// Create and show the highlight window on the main thread
fn create_highlight_window(x: i32, y: i32, width: i32, height: i32) {
    // Get main thread marker - we should be on main thread now
    let mtm = match MainThreadMarker::new() {
        Some(m) => m,
        None => {
            eprintln!("[macOS] Highlight: not on main thread");
            return;
        }
    };

    // Close any existing highlight window first
    {
        let mut current = CURRENT_HIGHLIGHT.lock().unwrap();
        if let Some(old_ptr) = current.take() {
            // Reconstruct the Retained to properly close and release the old window
            let old_window: Option<Retained<NSWindow>> =
                unsafe { Retained::from_raw(old_ptr as *mut NSWindow) };
            if let Some(w) = old_window {
                w.orderOut(None);
                // w is dropped here, releasing the window
            }
        }
    }

    // macOS coordinate system has origin at bottom-left, but we receive top-left coords
    // We need to flip the y coordinate using main screen height
    let screen_height = {
        let screens: Retained<NSArray<NSScreen>> = NSScreen::screens(mtm);
        if screens.count() > 0 {
            let main_screen = screens.objectAtIndex(0);
            main_screen.frame().size.height
        } else {
            1080.0 // fallback
        }
    };

    // Flip Y coordinate (convert from top-left to bottom-left origin)
    let flipped_y = screen_height - (y as f64) - (height as f64);

    let frame = NSRect::new(
        NSPoint::new(x as f64, flipped_y),
        NSSize::new(width as f64, height as f64),
    );

    // Create a borderless, transparent window
    let style = NSWindowStyleMask::Borderless;

    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };

    // Configure window properties
    // Make it float above everything (screen saver level is very high)
    window.setLevel(25);

    // Make it transparent
    window.setOpaque(false);
    window.setBackgroundColor(Some(&NSColor::clearColor()));

    // Make it click-through (ignore mouse events)
    window.setIgnoresMouseEvents(true);

    // Create a custom view that draws the border
    let content_view = create_border_view(mtm, width as CGFloat, height as CGFloat);
    window.setContentView(Some(&content_view));

    // Show the window
    window.orderFrontRegardless();

    // Store the window pointer in our static so it stays alive
    // Convert to usize to make it Send-safe
    let window_ptr = Retained::into_raw(window) as usize;
    {
        let mut current = CURRENT_HIGHLIGHT.lock().unwrap();
        *current = Some(window_ptr);
    }

    // Schedule window to close after duration
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(HIGHLIGHT_DURATION_MS));
        let queue = dispatch::Queue::main();
        queue.exec_async(move || {
            // Close the window on main thread
            let mut current = CURRENT_HIGHLIGHT.lock().unwrap();
            if let Some(ptr) = current.take() {
                // Reconstruct the Retained to properly close and release
                let window: Option<Retained<NSWindow>> =
                    unsafe { Retained::from_raw(ptr as *mut NSWindow) };
                if let Some(w) = window {
                    w.orderOut(None);
                    // Window is dropped here, properly releasing it
                }
            }
        });
    });
}

/// Create a view that draws a border using layer-backed drawing
fn create_border_view(mtm: MainThreadMarker, width: CGFloat, height: CGFloat) -> Retained<NSView> {
    unsafe {
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));

        let view = NSView::initWithFrame(NSView::alloc(mtm), frame);

        // Enable layer backing
        view.setWantsLayer(true);

        if let Some(layer) = view.layer() {
            // Set border color (nice blue: #2196F3)
            let cg_color = core_graphics::color::CGColor::rgb(
                0x21 as CGFloat / 255.0,
                0x96 as CGFloat / 255.0,
                0xF3 as CGFloat / 255.0,
                1.0,
            );

            // Get the raw CGColorRef pointer for passing to CALayer
            let cg_color_ref: *const c_void = cg_color.as_concrete_TypeRef() as *const c_void;

            // Set layer properties via objc message sends
            // CALayer methods: setBorderColor:, setBorderWidth:, setCornerRadius:
            // We need to use raw pointers since CGColorRef isn't Encode-compatible
            let layer_ptr: *const AnyObject = &*layer as *const _ as *const AnyObject;
            let layer_ref: &AnyObject = &*layer_ptr;

            let _: () = msg_send![layer_ref, setBorderColor: cg_color_ref];
            let _: () = msg_send![layer_ref, setBorderWidth: BORDER_WIDTH];
            let _: () = msg_send![layer_ref, setCornerRadius: 4.0f64];
        }

        view
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_highlight_does_not_panic() {
        // Just verify it doesn't crash (actual display requires main thread)
        show_highlight(100, 100, 800, 600);
        thread::sleep(Duration::from_millis(100));
    }
}
