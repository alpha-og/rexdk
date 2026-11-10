use crate::runtime::api::data::{DataMessageError, error::DecodeError};

/// REJECT message opcode
pub const REJECT_OPCODE: u8 = 0x82;

/// Expected REJECT message length, in bytes.
pub const REJECT_LEN: usize = 2;

/// A REJECT message sent in answer to a refused handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reject(RejectReason);

impl Reject {
    /// The reason the server refused the handshake.
    pub fn reason(&self) -> RejectReason {
        self.0
    }
}

impl TryFrom<&[u8]> for Reject {
    type Error = DataMessageError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != REJECT_LEN {
            return Err(DecodeError::InvalidLength {
                expected: REJECT_LEN,
                actual: value.len(),
            }
            .into());
        }
        if value[0] != REJECT_OPCODE {
            return Err(DecodeError::InvalidOpcode {
                expected: REJECT_OPCODE,
                actual: value[0],
            }
            .into());
        }
        let reason = value[1];
        Ok(Self(reason.into()))
    }
}

/// The reason the server refused a data connection handshake.
///
/// The server sends this value as the second byte of a `REJECT` message
/// (`0x82`), following the message opcode.
///
/// Unknown reason codes must be treated as failures. Their original byte
/// value is preserved in [`RejectReason::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// `0x01`: The ticket or token is unknown, expired, already used,
    /// issued to someone else, or its session no longer exists.
    InvalidCredentialOrSession,

    /// `0x02`: The client already has a connection to the same data source
    /// of the block. A client can have only one such connection at a time.
    /// If the previous connection dropped, use `RESUME` to replace it.
    DuplicateConnection,

    /// `0x03`: The stream is unavailable, such as when its block has closed,
    /// or a `RESUME` request reports fewer bytes received than the server sent.
    StreamUnavailable,

    /// `0x04`: Opening the connection would exceed a server limit.
    /// Retry later. The documented server does not currently send this reason.
    ConnectionLimit,

    /// `0x05`: The handshake was malformed, did not arrive in time,
    /// or a `RESUME` request reports more bytes received than the server sent.
    InvalidHandshake,

    /// `0x06`: The server encountered a failure.
    ServerFailure,

    /// `0x07`: The ticket or token was issued for a different subprotocol.
    WrongSubprotocol,

    /// An unrecognized rejection reason.
    ///
    /// The original reason byte is retained for diagnostics and
    /// forward compatibility. Treat this as a handshake failure.
    Unknown(u8),
}

impl From<u8> for RejectReason {
    fn from(value: u8) -> Self {
        match value {
            0x01 => Self::InvalidCredentialOrSession,
            0x02 => Self::DuplicateConnection,
            0x03 => Self::StreamUnavailable,
            0x04 => Self::ConnectionLimit,
            0x05 => Self::InvalidHandshake,
            0x06 => Self::ServerFailure,
            0x07 => Self::WrongSubprotocol,
            code => Self::Unknown(code),
        }
    }
}

impl From<RejectReason> for u8 {
    fn from(reason: RejectReason) -> Self {
        match reason {
            RejectReason::InvalidCredentialOrSession => 0x01,
            RejectReason::DuplicateConnection => 0x02,
            RejectReason::StreamUnavailable => 0x03,
            RejectReason::ConnectionLimit => 0x04,
            RejectReason::InvalidHandshake => 0x05,
            RejectReason::ServerFailure => 0x06,
            RejectReason::WrongSubprotocol => 0x07,
            RejectReason::Unknown(code) => code,
        }
    }
}

impl std::fmt::Display for RejectReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidCredentialOrSession => {
                "ticket or reconnect token is invalid, expired, or unusable"
            }
            Self::DuplicateConnection => "a connection to this data source already exists",
            Self::StreamUnavailable => "the stream is no longer available",
            Self::ConnectionLimit => "opening the connection would exceed a server limit",
            Self::InvalidHandshake => "the handshake is malformed or invalid",
            Self::ServerFailure => "the server encountered an internal failure",
            Self::WrongSubprotocol => {
                "the ticket or reconnect token belongs to another subprotocol"
            }
            Self::Unknown(code) => return write!(f, "unknown rejection reason: {code:#04x}"),
        };

        f.write_str(message)
    }
}
