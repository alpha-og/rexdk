//! Connection setup, handshakes, and long-lived connection tasks.
//!
//! The `protocol` module describes protocol messages; this module owns the I/O
//! that speaks them. Control and data connections share the transport timeout
//! and the HELLO/RESUME timeout defined here.

mod control;
mod data;
mod error;

use std::time::Duration;

use tokio::time::timeout;
use tokio_tungstenite::tungstenite;

use crate::runtime::transport::{self, Stream, Transport, TransportError};

pub(crate) use control::ControlHandle;
pub(crate) use data::DataConnectContext;
pub use data::{DATA_ENDPOINT, DataStreamHandle};
pub use error::{
    ConnectionContext, ConnectionError, ConnectionErrorKind, ConnectionPhase, ControlError,
    DataError,
};

/// Time allowed for HELLO/RESUME and the server's answer.
pub(crate) const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Time allowed for a transport to open.
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Opens a transport to `endpoint` within [`CONNECT_TIMEOUT`].
///
/// Control connections and every data-connection resume use this same entry
/// point, so timeouts and transport errors behave identically.
pub(crate) async fn connect(
    transport: &Transport,
    endpoint: &str,
    subprotocol: &str,
) -> Result<Stream, TransportError> {
    match timeout(
        CONNECT_TIMEOUT,
        transport::connect(transport, endpoint, subprotocol),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(TransportError::Timeout {
            endpoint: endpoint.to_owned(),
            seconds: CONNECT_TIMEOUT.as_secs(),
        }),
    }
}

/// Wraps a WebSocket failure with the endpoint from `context`.
pub(crate) fn websocket_error(
    context: &ConnectionContext,
    source: tungstenite::Error,
) -> TransportError {
    TransportError::WebSocket {
        endpoint: context.endpoint.clone(),
        source,
    }
}
