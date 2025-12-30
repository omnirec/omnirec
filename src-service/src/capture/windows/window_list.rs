//! Window enumeration using Windows API.

use crate::capture::WindowInfo;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::System::ProcessStatus::GetModuleBaseNameW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsIconic, IsWindowVisible, GA_ROOTOWNER, GetAncestor, GetWindow, GetWindowLongW,
    GWL_EXSTYLE, GW_OWNER, WS_EX_TOOLWINDOW,
};

/// List all visible, capturable windows.
pub fn list_windows() -> Vec<WindowInfo> {
    let mut windows: Vec<WindowInfo> = Vec::new();

    unsafe {
        let _ = EnumWindows(
            Some(enum_window_callback),
            LPARAM(&mut windows as *mut Vec<WindowInfo> as isize),
        );
    }

    windows
}

/// Callback for EnumWindows that filters and collects window info.
unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let windows = &mut *(lparam.0 as *mut Vec<WindowInfo>);

    // Skip invisible windows
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    // Skip minimized windows
    if IsIconic(hwnd).as_bool() {
        return BOOL(1);
    }

    // Skip windows without titles
    let title_len = GetWindowTextLengthW(hwnd);
    if title_len == 0 {
        return BOOL(1);
    }

    // Skip tool windows (tooltips, etc.)
    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
    if (ex_style as u32 & WS_EX_TOOLWINDOW.0) != 0 {
        return BOOL(1);
    }

    // Skip windows that are owned by another window (child windows)
    if let Ok(owner) = GetWindow(hwnd, GW_OWNER) {
        if !owner.0.is_null() {
            return BOOL(1);
        }
    }

    // Skip if not a root owner window
    let root_owner = GetAncestor(hwnd, GA_ROOTOWNER);
    if root_owner != hwnd {
        return BOOL(1);
    }

    // Get window title
    let mut title_buf: Vec<u16> = vec![0; (title_len + 1) as usize];
    let title_read = GetWindowTextW(hwnd, &mut title_buf);
    if title_read == 0 {
        return BOOL(1);
    }
    let title = OsString::from_wide(&title_buf[..title_read as usize])
        .to_string_lossy()
        .to_string();

    // Skip certain system windows by title
    let skip_titles = [
        "Program Manager",
        "Windows Input Experience",
        "Settings",
        "Microsoft Text Input Application",
    ];
    if skip_titles.iter().any(|t| title == *t) {
        return BOOL(1);
    }

    // Get process name
    let mut process_id: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut process_id));

    let process_name = if process_id != 0 {
        if let Ok(process_handle) =
            OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id)
        {
            let mut name_buf: Vec<u16> = vec![0; 260];
            let name_len = GetModuleBaseNameW(process_handle, None, &mut name_buf);
            if name_len > 0 {
                OsString::from_wide(&name_buf[..name_len as usize])
                    .to_string_lossy()
                    .to_string()
            } else {
                String::from("Unknown")
            }
        } else {
            String::from("Unknown")
        }
    } else {
        String::from("Unknown")
    };

    // Get window position and size
    let mut rect = RECT::default();
    let (x, y, width, height) = if GetWindowRect(hwnd, &mut rect).is_ok() {
        (
            rect.left,
            rect.top,
            (rect.right - rect.left).max(0) as u32,
            (rect.bottom - rect.top).max(0) as u32,
        )
    } else {
        (0, 0, 0, 0)
    };

    windows.push(WindowInfo {
        handle: hwnd.0 as isize,
        title,
        process_name,
        x,
        y,
        width,
        height,
    });

    BOOL(1) // Continue enumeration
}
