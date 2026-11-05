#[derive(Debug, thiserror::Error)]
pub enum HandshakeError {
    #[error("Connection closed")]
    ConnectionClosed,
    #[error("Handshake rejected: {0}")]
    Rejected(String),
    #[error("Unexpected message during handshake")]
    UnexpectedMessage,
    #[error("Invalid welcome message: {0}")]
    InvalidWelcome(String),
    #[error("Handshake timed out: {0}")]
    Timeout(String),
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
}
