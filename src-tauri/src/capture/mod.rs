//! Window capture module for enumerating and capturing windows.

mod windows_list;
pub mod recorder;

pub use windows_list::{list_windows, WindowInfo};
