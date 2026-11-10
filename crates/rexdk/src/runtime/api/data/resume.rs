use bytes::Bytes;

use crate::runtime::api::data::TICKET_LEN;

/// RESUME message opcode.
pub const RESUME_OPCODE: u8 = 0x02;

/// Expected RESUME message length.
pub const RESUME_LEN: usize = 25;

/// A client RESUME, reopening a dropped data stream.
///
/// RESUME is exactly 25 bytes: `0x02`, the latest 16-byte reconnect token, and
/// the number of stream bytes the client has received as a 64-bit
/// little-endian integer. The count must be exact, and it excludes the WELCOME
/// and any bytes a WebSocket library read ahead but the client has not used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resume {
    ticket: [u8; TICKET_LEN],
    bytes_received: u64,
}

impl Resume {
    /// Builds a RESUME for a reconnect token and the bytes received so far.
    pub fn new(ticket: [u8; TICKET_LEN], bytes_received: u64) -> Self {
        Self {
            ticket,
            bytes_received,
        }
    }
}

impl From<Resume> for Bytes {
    fn from(resume: Resume) -> Self {
        let mut bytes = Vec::with_capacity(RESUME_LEN);
        bytes.push(RESUME_OPCODE);
        bytes.extend_from_slice(&resume.ticket);
        bytes.extend(&resume.bytes_received.to_le_bytes());
        Self::from(bytes)
    }
}
