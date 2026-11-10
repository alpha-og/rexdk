//! Rex wire messages, split by connection.
//!
//! [`ControlMessage`] covers the JSON text messages on `/api/control`;
//! [`DataMessage`] covers the binary handshake frames on `/api/data`. The
//! top-level [`Message`] only groups the two families and has no wire encoding
//! of its own.

pub(crate) mod control;
pub mod data;
pub mod events;
mod message;

pub use control::ControlMessage;
pub use data::DataMessage;
#[allow(unused_imports)] // organization type; messages are parsed by family
pub use message::Message;
