use tokio_tungstenite::tungstenite::{error::ProtocolError, http};

use super::{
    api::{HandshakeError, ResponseError},
    disconnect::DisconnectReason,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to resolve the Rex server socket path")]
    SocketPathResolutionFailed,

    #[error("API error: {0}")]
    Api(#[from] ResponseError),

    #[error("Request ID space exhausted")]
    RequestIdExhausted,

    #[error("Message is {size} bytes, above the server limit of {limit} bytes")]
    MessageTooLarge { size: usize, limit: usize },

    #[error("Client is not connected; call connect() first")]
    NotConnected,

    #[error("Response channel closed before receiving a response")]
    ResponseChannelClosed,

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Failed to serialize or deserialize JSON: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error(transparent)]
    Connection(#[from] ConnectionError),

    #[error("Configuration error: {0}")]
    Configuration(#[source] anyhow::Error),

    #[error("Event channel closed")]
    EventChannelClosed,

    #[error("Event stream already taken")]
    EventStreamTaken,

    #[error("Internal error: {0}")]
    Internal(#[source] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("Connection closed: {0}")]
    Disconnected(DisconnectReason),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unsupported transport")]
    UnsupportedTransport { transport: String, platform: String },

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("WebSocket protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    #[error("WebSocket handshake failed: {0}")]
    Handshake(#[from] HandshakeError),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Connection(ConnectionError::Io(err))
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for Error {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::Connection(ConnectionError::WebSocket(err))
    }
}

impl From<HandshakeError> for Error {
    fn from(err: HandshakeError) -> Self {
        Self::Connection(ConnectionError::Handshake(err))
    }
}

impl From<std::env::VarError> for Error {
    fn from(err: std::env::VarError) -> Self {
        Self::Configuration(err.into())
    }
}

impl From<http::header::InvalidHeaderValue> for Error {
    fn from(err: http::header::InvalidHeaderValue) -> Self {
        Self::Configuration(err.into())
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err)
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for Error {
    fn from(_: tokio::sync::oneshot::error::RecvError) -> Self {
        Self::ResponseChannelClosed
    }
}
impl From<tokio::sync::mpsc::error::SendError<super::events::EventMessage>> for Error {
    fn from(_: tokio::sync::mpsc::error::SendError<super::events::EventMessage>) -> Self {
        Self::EventChannelClosed
    }
}
