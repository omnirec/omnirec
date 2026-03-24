//! Commands for opening auxiliary application windows.

/// Open the settings window using the backend window builder.
///
/// This keeps window creation consistent with tray actions, including macOS-
/// specific native window configuration that is required for packaged builds.
#[tauri::command]
pub async fn open_config_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::tray::open_config_window(&app);
    Ok(())
}

/// Open the about window using the backend window builder.
///
/// This keeps window creation consistent with tray actions, including macOS-
/// specific native window configuration that is required for packaged builds.
#[tauri::command]
pub async fn open_about_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::tray::open_about_window(&app);
    Ok(())
}
