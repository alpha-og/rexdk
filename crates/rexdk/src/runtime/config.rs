use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{Transport, bindings, protocol::control::ClientKind};

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientConfig {
    pub kind: ClientKind,
    pub name: String,
    pub version: String,
    pub label: String,
    pub api_revision: u32,
    pub api_min_revision: u32,
    pub transport: Transport,
    /// Schedule for client-side pings. `None` disables them.
    pub keepalive: Option<KeepAlive>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            kind: ClientKind::default(),
            name: String::new(),
            version: String::new(),
            label: String::new(),
            api_revision: bindings::metadata::API_REVISION,
            api_min_revision: bindings::metadata::API_REVISION,
            transport: Transport::default(),
            keepalive: Some(KeepAlive::default()),
        }
    }
}

/// Keepalive settings for the control connection.
///
/// Rex pings the client every 120 seconds and closes the connection with
/// status 1001 after three unanswered pings. The client sends its own pings on
/// the same schedule so that it notices a server that has gone away, such as
/// after a laptop sleeps or a network path drops without a close frame.
///
/// WebSocket libraries answer pings automatically; this type only controls the
/// client's own schedule. A ping that is not answered within [`Self::timeout`]
/// counts as missed, and after [`Self::max_missed`] consecutive misses the
/// client closes the connection with [`crate::DisconnectReason::KeepAliveTimeout`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeepAlive {
    /// How long to wait between pings.
    pub interval: Duration,
    /// How long to wait for a pong before counting a ping as missed.
    pub timeout: Duration,
    /// Consecutive missed pings allowed before the connection is closed.
    pub max_missed: u32,
}

impl Default for KeepAlive {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(120),
            timeout: Duration::from_secs(60),
            max_missed: 3,
        }
    }
}
