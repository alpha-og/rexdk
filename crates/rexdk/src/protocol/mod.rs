//! Rex wire messages, split by connection.
//!
//! This module is wire-only: it defines message shapes but performs no I/O and
//! depends only on serde and simple byte helpers. [`ControlMessage`] covers the
//! JSON text messages on `/api/control`; [`DataMessage`] covers the binary
//! handshake frames on `/api/data`. The top-level [`Message`] only groups the
//! two families and has no wire encoding of its own.

pub(crate) mod control;
pub mod data;
mod message;

pub use control::ControlMessage;
pub use data::DataMessage;
#[allow(unused_imports)] // organization type; messages are parsed by family
pub use message::Message;
