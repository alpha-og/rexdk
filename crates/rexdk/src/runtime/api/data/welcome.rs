//! Data-connection WELCOME handshake message.
//!
//! WELCOME is the server's response to a successful data-connection handshake.
//! It is sent as a single binary WebSocket message.
//!
//! The message is exactly 32 bytes:
//!
//! | Offset | Size | Description |
//! |---|---:|---|
//! | 0 | 1 | Opcode: `0x81` |
//! | 1 | 16 | Reconnect token |
//! | 17 | 3 | Reserved bytes; must be zero |
//! | 20 | 4 | Resume-retention duration, in milliseconds |
//! | 24 | 8 | Channel nonce |
//!
//! All integers are unsigned and little-endian.
//!
//! The channel nonce is nonzero and identifies Rex-generated terminal control
//! sequences, distinguishing them from sequences emitted by the terminal
//! program.
//!
//! The reconnect token is sensitive and must not be logged or exposed.
//! Each WELCOME provides a new token; RESUME must use the latest token.

use bytes::Bytes;

use crate::runtime::api::data::{
    DataMessageError,
    error::{DecodeError, ProtocolError, WelcomeError},
};

/// WELCOME message opcode.
pub const WELCOME_OPCODE: u8 = 0x81;

/// Expected WELCOME message length, in bytes.
pub const WELCOME_LEN: usize = 32;

/// Reconnect token length, in bytes.
pub const RECONNECT_TOKEN_LEN: usize = 16;

/// Successful data-connection handshake response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Welcome {
    /// Token used to resume the data stream after a connection interruption.
    ///
    /// The token is sensitive and must not be logged or exposed.
    pub reconnect_token: [u8; RECONNECT_TOKEN_LEN],

    /// Duration for which the server retains the stream for resumption,
    /// in milliseconds.
    pub resume_timeout_ms: u32,

    /// Nonzero identifier used to distinguish Rex-generated terminal control
    /// sequences from sequences emitted by the terminal program.
    pub channel_nonce: u64,
}

impl TryFrom<Bytes> for Welcome {
    type Error = DataMessageError;
    fn try_from(bytes: Bytes) -> Result<Self, Self::Error> {
        if bytes.len() != WELCOME_LEN {
            return Err(DecodeError::InvalidLength {
                expected: WELCOME_LEN,
                actual: bytes.len(),
            }
            .into());
        }

        if bytes[0] != WELCOME_OPCODE {
            return Err(DecodeError::InvalidOpcode {
                expected: WELCOME_OPCODE,
                actual: bytes[0],
            }
            .into());
        }

        if bytes[17..20] != [0, 0, 0] {
            return Err(ProtocolError::Welcome(WelcomeError::NonZeroReservedBytes).into());
        }

        let reconnect_token = bytes[1..17].try_into().expect("validated WELCOME length");

        let resume_timeout_ms =
            u32::from_le_bytes(bytes[20..24].try_into().expect("validated WELCOME length"));

        let channel_nonce =
            u64::from_le_bytes(bytes[24..32].try_into().expect("validated WELCOME length"));

        if channel_nonce == 0 {
            return Err(ProtocolError::Welcome(WelcomeError::ZeroChannelNonce).into());
        }

        Ok(Self {
            reconnect_token,
            resume_timeout_ms,
            channel_nonce,
        })
    }
}
