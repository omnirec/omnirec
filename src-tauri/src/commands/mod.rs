//! Tauri command handlers organized by functionality.
//!
//! This module contains all the `#[tauri::command]` functions that expose
//! backend functionality to the frontend.

mod audio;
mod capture;
mod config;
mod platform;
mod recording;

pub use audio::*;
pub use capture::*;
pub use config::*;
pub use platform::*;
pub use recording::*;
