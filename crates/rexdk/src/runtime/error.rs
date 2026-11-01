use tokio_tungstenite::tungstenite::{error::ProtocolError, http};

use crate::runtime::control::{handshake::HandshakeError, response::ResponseError};

#[derive(Debug)]
pub enum RexError {
    Api(ResponseError),
    RequestIdExhausted,
    ResponseChannelClosed,
    Serialization(serde_json::Error),
    Connection(ConnectionError),
    Configuration(anyhow::Error),
    Internal(anyhow::Error),
}

#[derive(Debug)]
pub enum ConnectionError {
    ConnectionClosed,
    Io(std::io::Error),
    WebSocket(tokio_tungstenite::tungstenite::Error),
    Protocol(ProtocolError),
    Handshake(HandshakeError),
}

impl std::fmt::Display for RexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Internal(err) => write!(f, "Internal Error: {err}"),
            _ => todo!(),
        }
    }
}

impl std::error::Error for RexError {}

impl From<anyhow::Error> for RexError {
    fn from(value: anyhow::Error) -> Self {
        Self::Internal(value)
    }
}

impl From<serde_json::Error> for RexError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value)
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for RexError {
    fn from(value: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::Connection(ConnectionError::WebSocket(value))
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for RexError {
    fn from(_: tokio::sync::oneshot::error::RecvError) -> Self {
        Self::ResponseChannelClosed
    }
}

impl From<std::env::VarError> for RexError {
    fn from(err: std::env::VarError) -> Self {
        Self::Configuration(err.into())
    }
}

impl From<std::io::Error> for RexError {
    fn from(err: std::io::Error) -> Self {
        Self::Connection(ConnectionError::Io(err))
    }
}

impl From<http::header::InvalidHeaderValue> for RexError {
    fn from(err: http::header::InvalidHeaderValue) -> Self {
        Self::Configuration(err.into())
    }
}

impl From<HandshakeError> for RexError {
    fn from(value: HandshakeError) -> Self {
        Self::Connection(ConnectionError::Handshake(value))
    }
}

impl From<ResponseError> for RexError {
    fn from(err: ResponseError) -> Self {
        Self::Api(err)
    }
}
