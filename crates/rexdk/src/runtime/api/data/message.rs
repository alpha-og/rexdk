use bytes::Bytes;

use crate::runtime::api::data::{
    DataMessageError, Hello, Reject, Welcome, error::DecodeError, reject::REJECT_OPCODE,
    resume::Resume, welcome::WELCOME_OPCODE,
};

/// A data-connection message.
///
/// Data messages are binary WebSocket frames on `/api/data`. HELLO and RESUME
/// go from client to server; WELCOME and REJECT come back during the handshake.
/// After WELCOME, the server's binary frames are the stream itself, not
/// [`DataMessage`]s: join them in order and treat frame boundaries as
/// meaningless.
///
/// Each handshake message has a fixed layout:
///
/// | Message | Size | Layout |
/// |---|---|---|
/// | HELLO | 17 | `0x01`, then a 16-byte ticket |
/// | RESUME | 25 | `0x02`, then a 16-byte reconnect token and a 64-bit byte count |
/// | WELCOME | 32 | `0x81`, then a 16-byte token, 3 zero bytes, a 32-bit resume time, and a 64-bit nonce |
/// | REJECT | 2 | `0x82`, then a reason |
///
/// Integers are unsigned and little-endian. The server ignores bytes after the
/// fixed part of a HELLO or RESUME, and ignores any client message sent after
/// the WELCOME.
#[derive(Debug)]
pub enum DataMessage {
    /// Client HELLO. Sent directly rather than parsed back.
    #[allow(dead_code)]
    Hello(Hello),

    /// Client RESUME. Sent directly rather than parsed back.
    #[allow(dead_code)]
    Resume(Resume),

    /// Server WELCOME, sent instead of a REJECT when the handshake succeeds.
    Welcome(Welcome),

    /// Server REJECT, sent when the handshake is refused.
    Reject(Reject),

    /// A frame whose opcode this SDK does not recognize.
    Unknown,
}

impl TryFrom<Bytes> for DataMessage {
    type Error = DataMessageError;

    fn try_from(bytes: Bytes) -> Result<Self, Self::Error> {
        let opcode = bytes.first().copied().ok_or(DecodeError::InvalidLength {
            expected: 1,
            actual: 0,
        })?;

        match opcode {
            WELCOME_OPCODE => Ok(Self::Welcome(Welcome::try_from(bytes)?)),
            REJECT_OPCODE => Ok(Self::Reject(Reject::try_from(&bytes[..])?)),
            _ => Ok(Self::Unknown),
        }
    }
}
