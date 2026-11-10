//! Errors encountered while encoding or decoding data-connection
//! handshake messages.

use thiserror::Error;

use crate::protocol::data::reject::RejectReason;

/// Errors encountered while processing data-connection handshake messages.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DataMessageError {
    #[error(transparent)]
    Decode(#[from] DecodeError),

    #[error(transparent)]
    Protocol(#[from] ProtocolError),
}

/// Errors encountered while decoding the wire representation of a message.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecodeError {
    /// The message length does not match the expected length.
    #[error("invalid message length: expected {expected} bytes, received {actual}")]
    InvalidLength {
        /// Expected message length in bytes.
        expected: usize,

        /// Actual message length in bytes.
        actual: usize,
    },

    /// The message contains an unexpected opcode.
    #[error("unexpected opcode: expected {expected:#04x}, received {actual:#04x}")]
    InvalidOpcode {
        /// Expected protocol opcode.
        expected: u8,

        /// Received protocol opcode.
        actual: u8,
    },

    /// A ticket returned by `data.connect` is not valid base64url for 16 bytes.
    #[error("invalid data ticket: {0}")]
    InvalidTicket(String),
}

/// Errors encountered when a decoded message violates protocol requirements.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProtocolError {
    #[error(transparent)]
    Welcome(#[from] WelcomeError),

    #[error(transparent)]
    Reject(#[from] RejectError),
}

/// Errors specific to WELCOME messages.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WelcomeError {
    /// A reserved field contains a nonzero value.
    #[error("WELCOME message contains nonzero reserved bytes")]
    NonZeroReservedBytes,

    /// The WELCOME message contains a zero channel nonce.
    #[error("WELCOME message contains a zero channel nonce")]
    ZeroChannelNonce,
}

/// Errors specific to REJECT messages.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RejectError {
    /// The REJECT message contains an invalid reason code.
    #[error("REJECT message contains an invalid reason code: {0:?}")]
    InvalidReason(RejectReason),
}
