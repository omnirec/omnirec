//! OmniRec Common Library
//!
//! Shared types and IPC protocol for communication between the OmniRec client
//! and service components.

pub mod ipc;
pub mod logging;
pub mod security;
pub mod types;

pub use types::*;
