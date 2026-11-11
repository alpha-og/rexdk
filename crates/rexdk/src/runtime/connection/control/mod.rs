//! Control-connection lifecycle, handshake, and per-socket tasks.

mod connection;
mod handle;
mod handshake;

use std::sync::Arc;

use futures_util::stream::SplitSink;
use tokio_tungstenite::tungstenite;

use crate::runtime::transport::Stream;

pub const CONTROL_ENDPOINT: &str = "/api/control";
pub const CONTROL_SUBPROTOCOL: &str = "rex.control.v1";

pub type ControlWriter = Arc<tokio::sync::Mutex<SplitSink<Stream, tungstenite::Message>>>;

const EVENT_BUFFER_CAPACITY: usize = 64;

pub(crate) use handle::ControlHandle;
