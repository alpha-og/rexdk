//! Control-connection lifecycle, handshake, and per-socket tasks.

mod connection;
mod handle;
mod handshake;

use std::sync::Arc;

use futures_util::stream::SplitSink;
use tokio::sync::{mpsc, mpsc::error::TrySendError, watch};
use tokio_tungstenite::tungstenite;

use crate::DisconnectReason;
use crate::events::EventMessage;
use crate::runtime::transport::Stream;

pub const CONTROL_ENDPOINT: &str = "/api/control";
pub const CONTROL_SUBPROTOCOL: &str = "rex.control.v1";

pub type ControlWriter = Arc<tokio::sync::Mutex<SplitSink<Stream, tungstenite::Message>>>;

/// The event queue and its lag signal.
#[derive(Clone, Debug)]
pub(super) struct EventChannel {
    events: mpsc::Sender<EventMessage>,
    lagged: watch::Sender<Option<u64>>,
    capacity: usize,
}

impl EventChannel {
    /// Queues an event without blocking the reader.
    ///
    /// A full queue ends the connection instead of dropping the event: the
    /// consumer can then reconnect and resync to keep `seq` continuous.
    pub(super) fn send(&self, event: EventMessage) -> Result<(), DisconnectReason> {
        match self.events.try_send(event) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(event)) => {
                self.lagged.send_replace(Some(event.seq));
                Err(DisconnectReason::EventBufferFull {
                    capacity: self.capacity,
                })
            }
            // The consumer dropped the event stream, so nobody is listening.
            Err(TrySendError::Closed(_)) => Ok(()),
        }
    }

    pub(super) fn lagged(&self) -> Option<u64> {
        *self.lagged.borrow()
    }

    pub(super) fn reset_lag(&self) {
        self.lagged.send_replace(None);
    }
}

pub(crate) use handle::ControlHandle;
