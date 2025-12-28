//! Platform detection and Wayland/Hyprland-specific commands.
//!
//! Commands for detecting the current platform, desktop environment,
//! and managing Hyprland-specific window operations.

/// Get the current platform name.
/// Returns "macos", "linux", or "windows".
#[tauri::command]
pub fn get_platform() -> String {
    #[cfg(target_os = "macos")]
    {
        "macos".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        "linux".to_string()
    }
    #[cfg(target_os = "windows")]
    {
        "windows".to_string()
    }
}

/// Check if running on Hyprland compositor.
#[tauri::command]
pub fn is_hyprland() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Check if running on GNOME desktop environment.
#[tauri::command]
pub fn is_gnome() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_uppercase().contains("GNOME"))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Check if running on KDE Plasma desktop environment.
#[tauri::command]
pub fn is_kde() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_uppercase().contains("KDE"))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Check if running on COSMIC desktop environment.
#[tauri::command]
pub fn is_cosmic() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_uppercase().contains("COSMIC"))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Check if running on Cinnamon desktop environment (Linux Mint).
#[tauri::command]
pub fn is_cinnamon() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_uppercase().contains("X-CINNAMON"))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Get the current desktop environment name.
/// Returns: "gnome", "kde", "cosmic", "cinnamon", "hyprland", or "unknown".
#[tauri::command]
pub fn get_desktop_environment() -> String {
    #[cfg(target_os = "linux")]
    {
        if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
            return "hyprland".to_string();
        }
        if std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_uppercase().contains("GNOME"))
            .unwrap_or(false)
        {
            return "gnome".to_string();
        }
        if std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_uppercase().contains("KDE"))
            .unwrap_or(false)
        {
            return "kde".to_string();
        }
        if std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_uppercase().contains("COSMIC"))
            .unwrap_or(false)
        {
            return "cosmic".to_string();
        }
        if std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_uppercase().contains("X-CINNAMON"))
            .unwrap_or(false)
        {
            return "cinnamon".to_string();
        }
        "unknown".to_string()
    }
    #[cfg(not(target_os = "linux"))]
    {
        "unknown".to_string()
    }
}

// =============================================================================
// Hyprland-specific commands
// =============================================================================

/// Configure Hyprland window rules for the region selector.
/// This makes the region selector window floating and properly positioned.
#[cfg(target_os = "linux")]
#[tauri::command]
pub async fn configure_region_selector_window(window_label: String) -> Result<(), String> {
    eprintln!(
        "[configure_region_selector] Configuring Hyprland rules for window: {}",
        window_label
    );

    // Check if we're on Hyprland
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_err() {
        eprintln!("[configure_region_selector] Not on Hyprland, skipping");
        return Ok(());
    }

    // Use hyprctl to add window rules for the region selector
    // We need to match by title since we can't set a custom class in Tauri
    let rules = vec![
        // Make it floating (not tiled)
        "float,title:^(Region Selection)$",
        // No border/gaps for clean overlay
        "noborder,title:^(Region Selection)$",
        "noshadow,title:^(Region Selection)$",
        "noblur,title:^(Region Selection)$",
        // No rounding for sharp selection
        "rounding 0,title:^(Region Selection)$",
        // Treat as opaque to prevent blur effects underneath
        "opaque 1,title:^(Region Selection)$",
        // Disable animations
        "noanim,title:^(Region Selection)$",
    ];

    // Execute commands via hyprctl
    for rule in rules {
        let output = std::process::Command::new("hyprctl")
            .args(["keyword", "windowrulev2", rule])
            .output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    eprintln!("[configure_region_selector] Applied: {}", rule);
                } else {
                    let err = String::from_utf8_lossy(&result.stderr);
                    eprintln!(
                        "[configure_region_selector] Failed to apply rule: {} - {}",
                        rule, err
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "[configure_region_selector] Failed to execute hyprctl: {}",
                    e
                );
            }
        }
    }

    Ok(())
}

/// Stub for non-Linux platforms.
#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub async fn configure_region_selector_window(_window_label: String) -> Result<(), String> {
    Ok(())
}

/// Get the position of the region selector window from Hyprland.
/// This is needed because Tauri's outerPosition() returns (0,0) on Wayland.
#[cfg(target_os = "linux")]
#[tauri::command]
pub async fn get_region_selector_position() -> Result<(i32, i32, i32, i32), String> {
    use hyprland::data::Clients;
    use hyprland::shared::HyprData;

    // Query Hyprland for the region selector window
    let clients = Clients::get().map_err(|e| format!("Failed to get clients: {}", e))?;

    for client in clients {
        if client.title == "Region Selection" {
            eprintln!(
                "[get_region_selector_position] Found window at ({}, {}) size {}x{}",
                client.at.0, client.at.1, client.size.0, client.size.1
            );
            return Ok((
                client.at.0 as i32,
                client.at.1 as i32,
                client.size.0 as i32,
                client.size.1 as i32,
            ));
        }
    }

    Err("Region selector window not found".to_string())
}

/// Stub for non-Linux platforms.
#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub async fn get_region_selector_position() -> Result<(i32, i32, i32, i32), String> {
    Err("Only available on Linux".to_string())
}

/// Move the region selector window to a specific position (Hyprland only).
/// Wayland doesn't allow apps to position windows, so we use Hyprland IPC.
#[cfg(target_os = "linux")]
#[tauri::command]
pub async fn move_region_selector(x: i32, y: i32, width: i32, height: i32) -> Result<(), String> {
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_err() {
        return Ok(()); // Not on Hyprland, silently ignore
    }

    // First, find the window address
    use hyprland::data::Clients;
    use hyprland::shared::HyprData;

    let clients = Clients::get().map_err(|e| format!("Failed to get clients: {}", e))?;

    let window_address = clients
        .iter()
        .find(|c| c.title == "Region Selection")
        .map(|c| format!("address:{}", c.address));

    let Some(addr) = window_address else {
        return Err("Region selector window not found".to_string());
    };

    // Move the window using hyprctl dispatch
    // movewindowpixel exact x y,<window>
    let move_cmd = format!("exact {} {},{}", x, y, addr);
    let output = std::process::Command::new("hyprctl")
        .args(["dispatch", "movewindowpixel", &move_cmd])
        .output()
        .map_err(|e| format!("Failed to run hyprctl: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        eprintln!("[move_region_selector] movewindowpixel failed: {}", err);
    }

    // Resize the window using hyprctl dispatch
    // resizewindowpixel exact w h,<window>
    let resize_cmd = format!("exact {} {},{}", width, height, addr);
    let output = std::process::Command::new("hyprctl")
        .args(["dispatch", "resizewindowpixel", &resize_cmd])
        .output()
        .map_err(|e| format!("Failed to run hyprctl: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        eprintln!("[move_region_selector] resizewindowpixel failed: {}", err);
    }

    eprintln!(
        "[move_region_selector] Moved window to ({}, {}) size {}x{}",
        x, y, width, height
    );

    Ok(())
}

/// Stub for non-Linux platforms.
#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub async fn move_region_selector(_x: i32, _y: i32, _width: i32, _height: i32) -> Result<(), String> {
    Ok(())
}
