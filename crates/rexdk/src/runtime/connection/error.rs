//! Errors from Rex connections.
//!
//! A [`ConnectionError`] carries a [`ConnectionContext`] describing the
//! endpoint and the phase the connection was in, plus a
//! [`ConnectionErrorKind`] separating control and data failures. Transport
//! failures are defined in [`crate::runtime::transport::TransportError`] and
//! wrapped by the corresponding kind.

use std::fmt;

use crate::{
    protocol::data::{DataMessageError, RejectReason},
    runtime::{disconnect::DisconnectReason, transport::TransportError},
};

/// The point in a connection's life when an error happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionPhase {
    /// Opening the transport and performing HELLO/WELCOME.
    Handshake,
    /// Reopening a dropped data connection with RESUME.
    Resume,
    /// Reading or writing an established connection.
    Stream,
}

impl fmt::Display for ConnectionPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Handshake => "handshake",
            Self::Resume => "resume",
            Self::Stream => "stream",
        })
    }
}

/// Identifies the connection an error belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionContext {
    /// Endpoint the connection targeted, such as `/api/control`.
    pub endpoint: String,
    /// Phase the connection was in when the error happened.
    pub phase: ConnectionPhase,
}

impl ConnectionContext {
    /// Builds a context for a connection endpoint and phase.
    pub fn new(endpoint: impl Into<String>, phase: ConnectionPhase) -> Self {
        Self {
            endpoint: endpoint.into(),
            phase,
        }
    }

    /// Returns the same connection context in another phase.
    pub fn with_phase(mut self, phase: ConnectionPhase) -> Self {
        self.phase = phase;
        self
    }
}

impl fmt::Display for ConnectionContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at {} during {}", self.endpoint, self.phase)
    }
}

/// A failure on a Rex connection, with the connection it belongs to.
#[derive(Debug)]
pub struct ConnectionError {
    /// Endpoint and phase of the connection that failed.
    pub context: ConnectionContext,
    /// Whether this is a control or data connection failure.
    pub kind: ConnectionErrorKind,
}

impl ConnectionError {
    /// Builds a connection error from its context and kind.
    pub fn new(context: ConnectionContext, kind: ConnectionErrorKind) -> Self {
        Self { context, kind }
    }
}

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ConnectionErrorKind::Control(error) => {
                write!(f, "control connection {}: {error}", self.context)
            }
            ConnectionErrorKind::Data(error) => {
                write!(f, "data connection {}: {error}", self.context)
            }
        }
    }
}

impl std::error::Error for ConnectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.kind)
    }
}

/// The kind of connection that failed.
#[derive(Debug, thiserror::Error)]
pub enum ConnectionErrorKind {
    /// The failure happened on the control connection.
    #[error(transparent)]
    Control(#[from] ControlError),

    /// The failure happened on a data connection.
    #[error(transparent)]
    Data(#[from] DataError),
}

/// A failure on the control connection.
#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    /// The transport could not be opened, or the WebSocket failed.
    #[error("transport failed: {0}")]
    Transport(#[source] TransportError),

    /// The connection ended while requests were still pending.
    #[error("connection ended: {0}")]
    Disconnected(DisconnectReason),

    /// The server closed the connection before the handshake completed.
    #[error("connection closed before the handshake completed")]
    Closed,

    /// The server did not answer in time.
    #[error("timed out after {seconds} seconds")]
    Timeout {
        /// Timeout that elapsed.
        seconds: u64,
    },

    /// The handshake received something other than the expected message.
    #[error("unexpected {0}")]
    UnexpectedMessage(String),

    /// The handshake message was malformed or invalid.
    #[error("handshake failed: {0}")]
    InvalidHandshake(String),
}

/// A failure on a data connection.
#[derive(Debug, thiserror::Error)]
pub enum DataError {
    /// The transport could not be opened, or the WebSocket failed.
    #[error("transport failed: {0}")]
    Transport(#[source] TransportError),

    /// The server did not answer a HELLO or RESUME in time.
    #[error("timed out after {seconds} seconds")]
    Timeout {
        /// Timeout that elapsed.
        seconds: u64,
    },

    /// The handshake ended without a valid WELCOME.
    #[error("handshake failed: {0}")]
    Handshake(String),

    /// The server refused the data-connection handshake.
    #[error("server rejected the connection: {0}")]
    Rejected(RejectReason),

    /// The server closed the connection and the stream cannot be resumed.
    #[error("connection closed with status {code}: {reason}")]
    Closed {
        /// WebSocket close status.
        code: u16,
        /// Human-readable reason sent with the close frame.
        reason: String,
    },

    /// The server requires a fresh ticket and HELLO to continue the stream.
    #[error("a new connection is required (status {code}: {reason})")]
    ReconnectRequired {
        /// WebSocket close status.
        code: u16,
        /// Human-readable reason sent with the close frame.
        reason: String,
    },

    /// The stream is no longer retained by the server for resumption.
    #[error("the stream is no longer available for resumption")]
    ResumeWindowElapsed,

    /// A stream message is larger than the data protocol allows.
    #[error("data frame is {size} bytes, above the protocol limit of {limit} bytes")]
    MessageTooLarge {
        /// Size of the frame, in bytes.
        size: usize,
        /// Largest frame the protocol allows, in bytes.
        limit: usize,
    },

    /// `data.connect` returned a ticket without naming a subprotocol.
    #[error("data.connect did not specify a subprotocol for the data connection")]
    MissingSubprotocol,

    /// A handshake or ticket message was malformed.
    #[error("malformed data connection message: {0}")]
    Protocol(#[from] DataMessageError),
}
