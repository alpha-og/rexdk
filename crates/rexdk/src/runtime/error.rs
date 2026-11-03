use thiserror::Error;
use tokio_tungstenite::tungstenite::{error::ProtocolError, http};

use super::api::{HandshakeError, ResponseError};

#[derive(Debug, Error)]
pub enum RexError {
    #[error("failed to resolve the Rex server socket path")]
    SocketPathResolutionFailed,

    #[error("API error: {0}")]
    Api(#[from] ResponseError),

    #[error("Request ID space exhausted")]
    RequestIdExhausted,

    #[error("Response channel closed before receiving a response")]
    ResponseChannelClosed,

    #[error("Failed to serialize or deserialize JSON: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error(transparent)]
    Connection(#[from] ConnectionError),

    #[error("Configuration error: {0}")]
    Configuration(#[source] anyhow::Error),

    #[error("Internal error: {0}")]
    Internal(#[source] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("Connection closed")]
    ConnectionClosed,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("WebSocket protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    #[error("WebSocket handshake failed: {0}")]
    Handshake(#[from] HandshakeError),
}

impl From<std::io::Error> for RexError {
    fn from(err: std::io::Error) -> Self {
        Self::Connection(ConnectionError::Io(err))
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for RexError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::Connection(ConnectionError::WebSocket(err))
    }
}

impl From<HandshakeError> for RexError {
    fn from(err: HandshakeError) -> Self {
        Self::Connection(ConnectionError::Handshake(err))
    }
}

impl From<std::env::VarError> for RexError {
    fn from(err: std::env::VarError) -> Self {
        Self::Configuration(err.into())
    }
}

impl From<http::header::InvalidHeaderValue> for RexError {
    fn from(err: http::header::InvalidHeaderValue) -> Self {
        Self::Configuration(err.into())
    }
}

impl From<anyhow::Error> for RexError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err)
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for RexError {
    fn from(_: tokio::sync::oneshot::error::RecvError) -> Self {
        Self::ResponseChannelClosed
    }
}
