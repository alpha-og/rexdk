//! Errors raised while opening a transport to the Rex server.
//!
//! These cover the socket itself and the WebSocket upgrade. Handshake and
//! connection-lifecycle failures belong in the connection module.

use tokio_tungstenite::tungstenite;

/// A failure to open a transport to the Rex server.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// The operating system refused or failed the socket connection.
    #[error("failed to open the {transport} transport to {endpoint}: {source}")]
    Connect {
        /// Transport kind, such as `unix socket`, `tcp`, or `tailscale`.
        transport: String,
        /// Socket path or network address that was targeted.
        endpoint: String,
        /// Underlying I/O failure.
        #[source]
        source: std::io::Error,
    },

    /// The configured transport cannot be opened on this platform.
    #[error("the {transport} transport is not supported on {platform}")]
    Unsupported {
        /// Transport kind, such as `UnixSocket`.
        transport: String,
        /// Platform the client is running on.
        platform: String,
    },

    /// Opening the transport took longer than the client allows.
    #[error("timed out after {seconds} seconds opening {endpoint}")]
    Timeout {
        /// Endpoint the client tried to reach.
        endpoint: String,
        /// Timeout that elapsed.
        seconds: u64,
    },

    /// The WebSocket upgrade failed after the socket was connected.
    #[error("WebSocket handshake with {endpoint} failed: {source}")]
    WebSocket {
        /// Endpoint the client tried to reach.
        endpoint: String,
        /// Underlying WebSocket failure.
        #[source]
        source: tungstenite::Error,
    },

    /// The WebSocket request itself could not be built.
    #[error("invalid WebSocket request for {endpoint}: {source}")]
    InvalidRequest {
        /// Endpoint the request targeted.
        endpoint: String,
        /// Reason the request is invalid.
        #[source]
        source: tungstenite::Error,
    },

    /// The subprotocol is not a valid WebSocket header value.
    #[error("invalid WebSocket subprotocol {subprotocol:?}: {source}")]
    InvalidSubprotocol {
        /// Subprotocol that could not be encoded.
        subprotocol: String,
        /// Reason the subprotocol is invalid.
        #[source]
        source: tungstenite::http::header::InvalidHeaderValue,
    },
}
