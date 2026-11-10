use bytes::Bytes;

use crate::runtime::api::data::{
    DataMessageError, Hello, Reject, Welcome, error::DecodeError, reject::REJECT_OPCODE,
    resume::Resume, welcome::WELCOME_OPCODE,
};

#[derive(Debug)]
pub enum Message {
    /// Client message; built directly and sent rather than parsed.
    #[allow(dead_code)]
    Hello(Hello),
    /// Client message; built directly and sent rather than parsed.
    #[allow(dead_code)]
    Resume(Resume),
    Welcome(Welcome),
    Reject(Reject),
    Unknown,
}

impl TryFrom<Bytes> for Message {
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
