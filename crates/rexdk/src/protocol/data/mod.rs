//! Data-connection wire messages.
//!
//! A data connection opens at `/api/data` with the subprotocol returned by
//! `data.connect`. The client sends one HELLO (or RESUME after a drop) and the
//! server answers with WELCOME or REJECT. After WELCOME, binary frames are raw
//! stream bytes.

/// Largest binary message a data connection carries, in bytes.
///
/// The protocol promises one to 65,536 stream bytes per message.
pub const MAX_STREAM_MESSAGE_BYTES: usize = 65_536;

mod error;
mod hello;
mod message;
mod reject;
mod resume;
mod welcome;

pub use error::DataMessageError;
pub use hello::{Hello, TICKET_LEN, decode_ticket};
pub use message::DataMessage;
pub use reject::{Reject, RejectReason};
pub use resume::Resume;
pub use welcome::{RECONNECT_TOKEN_LEN, Welcome};
