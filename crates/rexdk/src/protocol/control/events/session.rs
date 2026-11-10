//! Session-scoped Rex events and their payloads.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::protocol::control::events::Principal;

/// Events scoped to a session.
///
/// These arrive inside a `session_event` transport message. Client attachment
/// and connection events in this enum concern a client's relationship to a
/// particular session; they are distinct from server-level client events.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum SessionEvent {
    /// The session's active window changed.
    ActiveWindowChanged(ActiveWindowChanged),

    /// A block closed and left the session.
    BlockClosed(BlockClosed),

    /// A block joined the session.
    BlockCreated(BlockCreated),

    /// A block emitted a creator-defined event.
    ///
    /// This is a known Rex event (`block_event`) whose inner event name and
    /// payload are defined by the block creator. `block_creator.list` exposes
    /// creator event schemas.
    BlockEvent(RawExtensionEvent),

    /// A block's owner changed.
    BlockOwnerChanged(BlockOwnerChanged),

    /// A client attached to the session.
    ClientConnected(ClientConnected),

    /// An attached client's connection was lost or restored.
    ///
    /// The client remains attached while disconnected. If it does not
    /// reconnect within 60 seconds, `ClientDisconnected` follows.
    ClientConnectionChanged(ClientConnectionChanged),

    /// A client detached from the session.
    ClientDisconnected(ClientDisconnected),

    /// The available block creators changed. Call `block_creator.list`.
    CreatorChanged,

    /// The session label changed.
    SessionLabelChanged(SessionLabelChanged),

    /// The session view changed. Fetch `session.view` only if this revision
    /// exceeds the highest revision already observed.
    SessionViewChanged(SessionViewChanged),

    /// A window left the session.
    WindowClosed(WindowClosed),

    /// A window was added to the session.
    WindowCreated(WindowCreated),

    /// A window's label changed.
    WindowLabelChanged(WindowLabelChanged),
}

/// A creator-defined block event.
///
/// `block_event` itself is a known Rex session event. The creator-specific
/// `name` and `payload` are dynamic and are not statically typed by this SDK.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawExtensionEvent {
    /// ID of the block that emitted the event.
    pub block_id: String,

    /// Creator that made the block, such as `com.superlogical.terminal`.
    ///
    /// Omitted when Rex does not include it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator_name: Option<String>,

    /// Creator-defined event name, such as `bell`.
    pub name: String,

    /// Creator-defined event fields, following the schema advertised by
    /// `block_creator.list`. Omitted when the event has no payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
}

/// The session's active window changed.
///
/// `window_id` is omitted when the session has no windows.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActiveWindowChanged {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
}

/// A block closed and left the session.
///
/// API calls naming this block fail from this point, although the block may
/// still be shutting down.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlockClosed {
    /// ID of the block that closed.
    pub block_id: String,
}

/// A block joined the session.
///
/// API calls can name the block from this point, even if it has not yet been
/// placed in a window.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlockCreated {
    /// ID of the new block.
    pub block_id: String,
}

/// A block's ownership changed.
///
/// Rex does not send this event when the block closes; `block_closed` covers
/// that case.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlockOwnerChanged {
    /// ID of the block whose owner changed.
    pub block_id: String,

    /// New owner client ID. Omitted when the block no longer has an owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,

    /// Previous owner client ID. Omitted when there was no previous owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,

    /// Reason for the ownership change:
    /// `create`, `take`, `request`, `disown`, `lost`, `disconnect`, or
    /// `restore`. Rex advises ignoring unknown values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// A client attached to a session.
///
/// `client_id`, `principal`, and `connected_at` correspond to the information
/// returned for that client by `session.list_clients`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientConnected {
    /// ID of the client that attached.
    pub client_id: String,

    /// Server-identified principal.
    pub principal: Principal,

    /// RFC 3339 timestamp of the session attachment.
    pub connected_at: String,
}

/// An attached client's connection dropped or came back.
///
/// The client remains attached during connection loss. If it does not
/// reconnect within 60 seconds, `client_disconnected` follows.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientConnectionChanged {
    /// ID of the client whose connection changed.
    pub client_id: String,

    /// Principal from the client's `client_connected` event.
    pub principal: Principal,

    /// `connected` when the connection came back, or `lost` when it dropped.
    ///
    /// Rex advises treating unknown values as `connected`.
    pub connection_state: String,

    /// RFC 3339 timestamp when the server noticed the connection loss.
    ///
    /// Set only while `connection_state` is `lost`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_lost_at: Option<String>,
}

/// A client detached from a session.
///
/// This can happen because the client called `session.detach`, left the server,
/// or the session was destroyed.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientDisconnected {
    /// ID of the client that detached.
    pub client_id: String,

    /// Principal from the client's `client_connected` event.
    pub principal: Principal,
}

/// The session's label changed.
///
/// When the label is removed, Rex sends the default label that the session
/// reverted to.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionLabelChanged {
    /// New session label, or the default label if the previous label was
    /// removed.
    pub label: String,
}

/// The session view changed.
///
/// Events can arrive out of revision order. Fetch `session.view` only when the
/// received revision is higher than the latest revision already observed.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionViewChanged {
    /// Session revision after the change.
    pub revision: u64,
}

/// A window left the session.
///
/// This is emitted when a window's last block closes, moves to another window,
/// or is detached. It is not emitted merely because the entire session was
/// destroyed.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WindowClosed {
    /// ID of the window that closed.
    pub window_id: String,
}

/// A new window was added to the session.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WindowCreated {
    /// ID of the new window.
    pub window_id: String,

    /// New window label.
    pub label: String,
}

/// A window's label changed.
///
/// When the label is removed, Rex sends the default label that the window
/// reverted to.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WindowLabelChanged {
    /// ID of the window whose label changed.
    pub window_id: String,

    /// New window label, or the default label if the previous label was
    /// removed.
    pub label: String,
}
