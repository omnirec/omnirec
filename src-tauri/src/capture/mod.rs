//! Window capture module for enumerating and capturing windows.

mod windows_list;
mod monitor_list;
pub mod recorder;
pub mod region_recorder;

pub use windows_list::{list_windows, WindowInfo};
pub use monitor_list::{list_monitors, MonitorInfo};
pub use region_recorder::CaptureRegion;
