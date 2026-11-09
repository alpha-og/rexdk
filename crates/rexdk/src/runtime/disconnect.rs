//! Why a Rex control connection ended.

use crate::{Error, runtime::error::ConnectionError};

/// Why a control connection ended.
///
/// Rex closes a control connection with a WebSocket status and reason. The
/// statuses a client is expected to handle are:
///
/// - `1001` `keepalive timeout`: the client stopped answering pings.
/// - `1008` `no common API revision: ...`: the client and server share no API
///   revision, so one of them must be updated.
/// - `1008` `event backlog exceeded`: the client fell more than 8 MiB behind.
///   Reconnect and read the state again.
/// - `1009` `read limited at ...` and `1009` `event exceeds message limit`: a
///   message was larger than the connection's limit.
///
/// A connection that drops without a close frame is reported as
/// [`DisconnectReason::Abnormal`], the WebSocket status `1006`. Rex keeps such
/// a client for 60 seconds, so [`crate::Client::reconnect`] can resume it
/// within that window.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DisconnectReason {
    /// The client closed the connection, either with
    /// [`crate::Client::disconnect`] or by dropping the client.
    #[error("closed by the client")]
    Local,

    /// The server closed the connection with a WebSocket close frame.
    #[error("server closed the connection with status {code}: {reason}")]
    Server {
        /// WebSocket close status, such as `1001` or `1008`.
        code: u16,
        /// Human-readable reason sent with the close frame.
        reason: String,
    },

    /// The connection dropped without a close frame (status `1006`).
    #[error("connection dropped without a close frame")]
    Abnormal,

    /// The client sent pings but the server stopped answering them.
    #[error("keepalive timeout")]
    KeepAliveTimeout,

    /// Reading from the connection failed.
    #[error("read failed: {0}")]
    ReadError(String),
}

impl DisconnectReason {
    /// Converts this reason into the error returned to callers whose requests
    /// were still pending when the connection ended.
    pub fn to_error(&self) -> Error {
        Error::Connection(ConnectionError::Disconnected(self.clone()))
    }
}
