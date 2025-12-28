//! IPC protocol for client-service communication.

mod protocol;
mod requests;
mod responses;

pub use protocol::*;
pub use requests::*;
pub use responses::*;
