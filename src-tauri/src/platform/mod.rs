//! Platform-specific functionality.
//!
//! This module contains minimal platform-specific code for checks that must be done
//! in the Tauri client (e.g., permission checks on macOS).

#[cfg(target_os = "macos")]
pub mod macos;
