//! Data-connection HELLO handshake message.
//!
//! HELLO is the initial client message sent after opening a WebSocket
//! connection to `/api/data`.
//!
//! The message is exactly 17 bytes:
//!
//! | Offset | Size | Description |
//! |---|---:|---|
//! | 0 | 1 | Opcode: `0x01` |
//! | 1..17 | 16 | Ticket |
//!
//! The ticket is obtained through `data.connect` on the control connection.
//! It is encoded as base64url without padding in the RPC response and must
//! be decoded before constructing this message.
//!
//! Send HELLO as a single binary WebSocket message within 5 seconds of
//! opening the data connection.
//!
//! A ticket is single-use and expires after the duration specified by
//! `data.connect`. Sending HELLO consumes the ticket, including when the
//! server rejects the handshake.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use bytes::Bytes;

use crate::runtime::api::data::{DataMessageError, error::DecodeError};

/// HELLO message opcode.
pub const HELLO_OPCODE: u8 = 0x01;

/// Expected HELLO message length.
pub const HELLO_LEN: usize = 17;

/// Number of bytes in a data-connection ticket.
pub const TICKET_LEN: usize = 16;

/// Initial data-connection handshake message.
///
/// The ticket must be decoded from the base64url-without-padding value
/// returned by `data.connect`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    /// Single-use ticket authorizing the data connection.
    ticket: [u8; TICKET_LEN],
}

impl Hello {
    pub fn new(ticket: [u8; TICKET_LEN]) -> Self {
        Self { ticket }
    }
}

/// Decodes a ticket returned by `data.connect`.
///
/// Tickets are encoded as base64url without padding, as 22 characters for the
/// 16 ticket bytes.
pub fn decode_ticket(encoded: &str) -> Result<[u8; TICKET_LEN], DataMessageError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|err| DecodeError::InvalidTicket(err.to_string()))?;

    bytes.try_into().map_err(|bytes: Vec<u8>| {
        DecodeError::InvalidTicket(format!(
            "expected {TICKET_LEN} bytes, decoded {}",
            bytes.len()
        ))
        .into()
    })
}

impl From<Hello> for Bytes {
    fn from(hello: Hello) -> Self {
        let mut bytes = Vec::with_capacity(HELLO_LEN);

        bytes.push(HELLO_OPCODE);
        bytes.extend_from_slice(&hello.ticket);

        Self::from(bytes)
    }
}
