//! Platform detection and handling.
//!
//! Detects Wayland portal-mode desktops where specific target selection
//! is not supported.

/// Check if we're running on a portal-mode desktop.
///
/// On these desktops, programmatic source selection is not possible.
/// The user must select via the desktop's native picker.
pub fn is_portal_mode_desktop() -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
            let desktop_upper = desktop.to_uppercase();
            return desktop_upper.contains("GNOME")
                || desktop_upper.contains("KDE")
                || desktop_upper.contains("COSMIC")
                || desktop_upper.contains("X-CINNAMON");
        }
        false
    }

    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Check if portal-based capture is supported on this platform.
pub fn is_portal_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        // Portal is supported on Linux Wayland desktops
        // Check if we're on Wayland
        std::env::var("WAYLAND_DISPLAY").is_ok()
            || std::env::var("XDG_SESSION_TYPE")
                .map(|t| t == "wayland")
                .unwrap_or(false)
    }

    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Get the name of the current desktop environment (for messages).
pub fn desktop_name() -> Option<String> {
    std::env::var("XDG_CURRENT_DESKTOP").ok()
}
