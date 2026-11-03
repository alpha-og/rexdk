#[derive(Debug)]
pub enum HandshakeError {
    ConnectionClosed,
    Rejected(String),
    UnexpectedMessage,
    InvalidWelcome(String),
    Timeout,
}

impl std::fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionClosed => write!(f, "Connection closed"),
            Self::Rejected(reason) => {
                write!(f, "Handshake rejected: {reason}")
            }
            Self::UnexpectedMessage => {
                write!(f, "Unexpected message during handshake")
            }
            Self::InvalidWelcome(reason) => {
                write!(f, "Invalid welcome message: {reason}")
            }
            Self::Timeout => {
                write!(f, "Handshake timed out")
            }
        }
    }
}

impl std::error::Error for HandshakeError {}
