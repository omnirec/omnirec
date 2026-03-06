//! OmniRec Types Library
//!
//! OmniRec-specific types, IPC protocol, logging, and security utilities
//! shared between the Tauri backend (src-tauri) and CLI (src-cli).

pub mod ipc;
pub mod logging;
pub mod security;
pub mod types;

pub use types::*;
