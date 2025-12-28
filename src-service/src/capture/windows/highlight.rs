//! Native display highlight using Windows APIs.
//!
//! Creates a transparent layered window with a colored border to highlight a monitor.
//! Uses UpdateLayeredWindow for flicker-free alpha animation.

use std::thread;
use std::time::Instant;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    BLENDFUNCTION, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, KillTimer,
    PostQuitMessage, RegisterClassW, SetTimer, ShowWindow, UpdateLayeredWindow, CS_HREDRAW,
    CS_VREDRAW, MSG, SW_SHOWNOACTIVATE, ULW_ALPHA, WM_DESTROY, WM_TIMER, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

const BORDER_WIDTH: i32 = 8;
const ANIMATION_DURATION_MS: u64 = 800;
const TIMER_ID: usize = 1;
const TIMER_INTERVAL_MS: u32 = 16; // ~60fps



/// Show a highlight border around the specified monitor area.
/// This function spawns a thread and returns immediately.
pub fn show_highlight(x: i32, y: i32, width: i32, height: i32) {
    thread::spawn(move || {
        unsafe {
            run_highlight_window(x, y, width, height);
        }
    });
}

/// Render state for UpdateLayeredWindow
struct RenderState {
    screen_dc: HDC,
    mem_dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    pt_dst: POINT,
    size: SIZE,
    pt_src: POINT,
    start_time: Instant,
}

thread_local! {
    static RENDER_STATE: std::cell::RefCell<Option<RenderState>> = const { std::cell::RefCell::new(None) };
}

unsafe fn run_highlight_window(x: i32, y: i32, width: i32, height: i32) {
    let class_name: Vec<u16> = "ScreenRecorderHighlight\0".encode_utf16().collect();
    let hmodule = GetModuleHandleW(PCWSTR::null()).unwrap_or_default();
    let hinstance = HINSTANCE(hmodule.0);

    // Register window class
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(highlight_wnd_proc),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };

    RegisterClassW(&wc);

    // Create layered window
    let hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT,
        PCWSTR(class_name.as_ptr()),
        PCWSTR::null(),
        WS_POPUP,
        x,
        y,
        width,
        height,
        HWND::default(),
        None,
        hinstance,
        None,
    )
    .unwrap_or_default();

    if hwnd.0.is_null() {
        return;
    }

    // Create the border bitmap with per-pixel alpha (32-bit ARGB)
    let screen_dc = GetDC(HWND::default());
    let mem_dc = CreateCompatibleDC(screen_dc);

    // Create 32-bit DIB section for per-pixel alpha
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // Top-down DIB
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [Default::default()],
    };

    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let bitmap = CreateDIBSection(
        screen_dc,
        &bmi,
        DIB_RGB_COLORS,
        &mut bits,
        None,
        0,
    ).unwrap_or_default();

    if bitmap.is_invalid() || bits.is_null() {
        let _ = DeleteDC(mem_dc);
        let _ = ReleaseDC(HWND::default(), screen_dc);
        return;
    }

    let old_bitmap = SelectObject(mem_dc, bitmap);

    // Draw directly to the pixel buffer
    // Windows DIB is BGRA in memory, and for AC_SRC_ALPHA we need premultiplied alpha
    // On little-endian, u32 0xAARRGGBB becomes bytes [BB, GG, RR, AA] in memory
    // But Windows DIB expects [BB, GG, RR, AA] which is u32 as 0xAARRGGBB... 
    // Actually, let's write bytes directly to be safe
    let pixels = bits as *mut u8;
    let stride = (width * 4) as usize; // 4 bytes per pixel

    // #2196F3 = R:0x21, G:0x96, B:0xF3
    let r: u8 = 0x21;
    let g: u8 = 0x96;
    let b: u8 = 0xF3;
    let a: u8 = 0xFF;

    for py in 0..height {
        for px in 0..width {
            let idx = (py as usize) * stride + (px as usize) * 4;
            let is_border = py < BORDER_WIDTH
                || py >= height - BORDER_WIDTH
                || px < BORDER_WIDTH
                || px >= width - BORDER_WIDTH;

            if is_border {
                // BGRA order, premultiplied (since alpha=255, RGB values stay the same)
                *pixels.add(idx) = b;     // Blue
                *pixels.add(idx + 1) = g; // Green
                *pixels.add(idx + 2) = r; // Red
                *pixels.add(idx + 3) = a; // Alpha
            } else {
                // Fully transparent
                *pixels.add(idx) = 0;
                *pixels.add(idx + 1) = 0;
                *pixels.add(idx + 2) = 0;
                *pixels.add(idx + 3) = 0;
            }
        }
    }

    // Store render state
    let pt_dst = POINT { x, y };
    let size = SIZE { cx: width, cy: height };
    let pt_src = POINT { x: 0, y: 0 };

    RENDER_STATE.with(|state| {
        *state.borrow_mut() = Some(RenderState {
            screen_dc,
            mem_dc,
            bitmap,
            old_bitmap,
            pt_dst,
            size,
            pt_src,
            start_time: Instant::now(),
        });
    });

    // Initial display with alpha = 0 (will fade in)
    update_window_alpha(hwnd, 0);

    // Show window without activating it (so it doesn't steal focus)
    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);

    // Start animation timer
    SetTimer(hwnd, TIMER_ID, TIMER_INTERVAL_MS, None);

    // Message loop
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
        DispatchMessageW(&msg);
    }

    // Cleanup
    RENDER_STATE.with(|state| {
        if let Some(rs) = state.borrow_mut().take() {
            SelectObject(rs.mem_dc, rs.old_bitmap);
            let _ = DeleteObject(rs.bitmap);
            let _ = DeleteDC(rs.mem_dc);
            let _ = ReleaseDC(HWND::default(), rs.screen_dc);
        }
    });
}

/// Update the window with a new alpha value (0-255)
unsafe fn update_window_alpha(hwnd: HWND, alpha: u8) {
    RENDER_STATE.with(|state| {
        if let Some(ref rs) = *state.borrow() {
            let blend = BLENDFUNCTION {
                BlendOp: 0,      // AC_SRC_OVER
                BlendFlags: 0,
                SourceConstantAlpha: alpha,
                AlphaFormat: 1,  // AC_SRC_ALPHA - use per-pixel alpha
            };

            let _ = UpdateLayeredWindow(
                hwnd,
                rs.screen_dc,
                Some(&rs.pt_dst),
                Some(&rs.size),
                rs.mem_dc,
                Some(&rs.pt_src),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
        }
    });
}

unsafe extern "system" fn highlight_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TIMER => {
            let elapsed = RENDER_STATE.with(|state| {
                state.borrow().as_ref().map(|s| s.start_time.elapsed())
            });

            if let Some(elapsed) = elapsed {
                let elapsed_ms = elapsed.as_millis() as u64;

                if elapsed_ms >= ANIMATION_DURATION_MS {
                    // Animation complete, close window
                    KillTimer(hwnd, TIMER_ID).ok();
                    DestroyWindow(hwnd).ok();
                } else {
                    // Calculate alpha based on animation progress
                    // Fade in for first 15%, hold until 70%, fade out
                    let progress = elapsed_ms as f32 / ANIMATION_DURATION_MS as f32;
                    let alpha = if progress < 0.15 {
                        // Fade in
                        (progress / 0.15 * 255.0) as u8
                    } else if progress < 0.70 {
                        // Hold
                        255
                    } else {
                        // Fade out
                        ((1.0 - (progress - 0.70) / 0.30) * 255.0) as u8
                    };

                    update_window_alpha(hwnd, alpha);
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
