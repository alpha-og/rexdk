use bytes::Bytes;

use crate::runtime::api::data::TICKET_LEN;

/// RESUME message opcode.
pub const RESUME_OPCODE: u8 = 0x02;

/// Expected RESUME message length.
pub const RESUME_LEN: usize = 25;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resume {
    ticket: [u8; TICKET_LEN],
    bytes_received: u64,
}

impl Resume {
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
