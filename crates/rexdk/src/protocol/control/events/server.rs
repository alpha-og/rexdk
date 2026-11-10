//! Server-scoped Rex events and their payloads.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::protocol::control::events::{Client, ClientReference};

/// Events about the Rex server and its clients.
///
/// These are server-scope events. Most are delivered to connections using
/// `session.attach_all` or `session.bootstrap` with `event_scope = all`, subject
/// to the individual event's delivery rules.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ServerEvent {
    /// Asks this client to call a method on another server and answer using
    /// `client.reply` with the same `call_id`, even if the call fails.
    ClientCallRequested(ClientCallRequested),

    /// A client changed its label/actions or its advertised information.
    ClientChanged(ClientChanged),

    /// A new client connected to the Rex server. Not emitted for reconnects.
    ClientJoined(ClientJoined),

    /// A client left the server, including after the reconnect timeout.
    ClientLeft(ClientLeft),

    /// Requests this client to perform an action. Reply promptly using
    /// `client.reply` with the same `call_id`.
    ClientPerformRequested(ClientPerformRequested),

    /// A `client.perform` request ended, regardless of whether it succeeded.
    ClientPerformed(ClientPerformed),

    /// Client Lua scripts missed session events.
    ClientScriptEventsDropped(ClientScriptEventsDropped),

    /// The server's host list changed. Call `hosts.list` unless the hash
    /// matches the last hash fetched.
    HostsChanged(HostsChanged),

    /// Server configuration was reloaded and the keymap may have changed.
    /// Call `client.keymap` unless its hash is unchanged.
    KeymapChanged(KeymapChanged),

    /// A session was created. Delivered to attach-all/bootstrap connections.
    SessionCreated(SessionCreated),

    /// A session was destroyed. This is its final event, except a
    /// `client_script_events_dropped` event may still follow.
    SessionDestroyed(SessionDestroyed),

    /// A session changed its position in the order returned by `session.list`.
    SessionMoved(SessionMoved),

    /// The server's built-in Tailscale status changed.
    TailscaleStatusChanged(TailscaleStatusChanged),
}

/// Asks a client to call a method on another server for a running
/// `client.action`.
///
/// The client must send `client.reply` with this event's `call_id`, including
/// when the method call fails. A connection never reuses a `call_id`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientCallRequested {
    /// ID to send back to `client.reply`.
    pub call_id: String,

    /// Server ID supplied in the `servers` list of the originating
    /// `client.action` call.
    pub server: String,

    /// Method to call, such as `session.set_label`.
    pub method: String,

    /// Method arguments. Omitted when there are none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Value>,
}

/// A client changed its advertised properties.
///
/// Rex emits this when the client changes its label or actions, or reconnects
/// and describes itself differently. Only `event_scope = all` connections
/// receive it.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientChanged {
    /// The client as it is now.
    pub client: Client,
}

/// A new client connected to the server.
///
/// Rex does not emit this for reconnects. Only `event_scope = all` connections
/// receive it.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientJoined {
    /// The client that connected.
    pub client: Client,
}

/// A client left the server.
///
/// Rex emits this when the client closes its connection, or when it loses its
/// connection and does not reconnect within 60 seconds. Only
/// `event_scope = all` connections receive it.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientLeft {
    /// ID of the client that left.
    pub client_id: String,
}

/// Requests a client to perform one of its registered actions for a
/// `client.perform` call.
///
/// The caller is waiting, and later requests to this client wait behind this
/// one, so the client should reply promptly using `client.reply` and this
/// event's `call_id`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientPerformRequested {
    /// ID to send back to `client.reply`. A connection never reuses one.
    pub call_id: String,

    /// Name of the action to perform.
    pub action: String,

    /// Action arguments, as a JSON object. Omitted if the request had none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Value>,

    /// Cause of the request, as supplied by the requester, such as `key` or
    /// `cli`. An empty string means `api`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,

    /// Session the action applies to, if specified by the request.
    ///
    /// Rex has not validated this ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Block the action applies to, if specified by the request.
    ///
    /// Rex has not validated this ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_id: Option<String>,

    /// Client that initiated the request. Rex derives this from the
    /// requester's connection, so it cannot be forged by the request.
    pub requester: ClientReference,
}

/// Result notification for a `client.perform` request.
///
/// This event means the request ended; it does not necessarily mean that the
/// action was performed successfully. Only `event_scope = all` connections
/// receive it.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientPerformed {
    /// Client that was asked to perform the action.
    pub client_id: String,

    /// Action name from the request.
    pub action: String,

    /// Client that initiated the request.
    pub requester: ClientReference,

    /// How the request ended: `performed`, `refused`, `failed`, or `timeout`.
    ///
    /// Rex advises treating unknown values as `failed`.
    pub outcome: String,

    /// Failure information. Present for every outcome except `performed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ClientActionError>,
}

/// Error details for a failed client action.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientActionError {
    /// Error code, such as `not_found`, `refused`, or `internal`.
    pub code: String,

    /// Human-readable description of what went wrong.
    pub message: String,
}

/// Notification that Lua scripts loaded by a client with `client.set_script`
/// fell behind and missed events from a session.
///
/// Later misses are not reported until the scripts catch up. The notification
/// is sent to clients attached to that session and to `event_scope = all`
/// connections.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientScriptEventsDropped {
    /// Session whose events the scripts missed.
    pub session_id: String,

    /// Client whose scripts fell behind.
    pub client_id: String,

    /// First missed event name, using the name passed to `rex.on`, such as
    /// `block_closed` or `com.superlogical.terminal.bell`.
    pub event_name: String,
}

/// Notification that the server's host list changed.
///
/// Call `hosts.list` unless the hash matches the most recently fetched host
/// list.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HostsChanged {
    /// Hash identifying the new host list.
    pub hash: String,
}

/// Notification that server configuration was reloaded.
///
/// Call `client.keymap` unless the hash matches the last keymap hash fetched
/// from this server.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeymapChanged {
    /// Number of configuration loads since server startup, including this one.
    ///
    /// The next `client.keymap` result reports this value in
    /// `config.generation`, unless another reload happens first.
    pub generation: u64,

    /// Configuration hash, as reported by `client.keymap` in `config.hash`.
    pub hash: String,
}

/// Notification that a session was created.
///
/// Connections that called `session.attach_all` or `session.bootstrap` receive
/// this event.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionCreated {
    /// New session, in the same shape as `session.list`.
    pub session: SessionSummary,
}

/// Session information as returned by `session.list`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionSummary {
    /// Session ID.
    pub session_id: String,

    /// Session label.
    pub label: String,
}

/// Notification that a session was destroyed.
///
/// This is the final event about that session, except that a
/// `client_script_events_dropped` event for it may still follow. It is sent to
/// connections attached to the session and to attach-all/bootstrap
/// connections.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionDestroyed {
    /// ID of the destroyed session.
    pub session_id: String,
}

/// Notification that a session moved in the ordering returned by
/// `session.list`.
///
/// Connections that called `session.attach_all` or `session.bootstrap`
/// receive this event.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionMoved {
    /// ID of the session that moved.
    pub session_id: String,

    /// ID of the session now immediately after the moved session.
    ///
    /// Omitted when the moved session is now last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_session_id: Option<String>,
}

/// Notification that the server's built-in Tailscale status changed.
///
/// This carries the complete new status. Only connections that called
/// `session.attach_all` or `session.bootstrap` receive it.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TailscaleStatusChanged {
    /// Why built-in Tailscale was disabled after its HTTPS listener failed.
    ///
    /// The next enable clears this error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Tailscale state: `disabled`, `starting`, `needs_login`,
    /// `needs_machine_approval`, `running`, or a future value.
    ///
    /// Rex advises treating unknown values as `starting`.
    pub state: String,

    /// Device MagicDNS name, such as `rex.tail1234.ts.net`.
    ///
    /// Omitted until known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns_name: Option<String>,

    /// Sign-in URL. Present while state is `needs_login`, once available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_url: Option<String>,

    /// Account that signed the device in. Omitted until Tailscale reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<TailscaleAccount>,

    /// Tailnet display name, or its domain if it has no display name.
    ///
    /// Omitted until known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tailnet_name: Option<String>,

    /// Whether tailnet members allowed by ACLs may connect.
    pub allow_tailnet_members: bool,

    /// Tailscale device tags, such as `tag:rex`.
    ///
    /// A tagged device has no owner. Rex omits this field when there are no
    /// tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// HTTPS listener state. Omitted while built-in Tailscale is off and by
    /// older servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub https: Option<TailscaleHttps>,

    /// Control server used by the device. Omitted while built-in Tailscale is off and by older
    /// servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_url: Option<String>,
}

/// Tailscale account information.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TailscaleAccount {
    /// Tailscale login name, usually an email address.
    ///
    /// Display-only because it can change.
    pub login_name: String,
}

/// State of the server's Tailscale HTTPS listener.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TailscaleHttps {
    /// `pending` until the HTTPS listener is ready, then `available`.
    ///
    /// Rex advises treating unknown values as `pending`.
    pub state: String,

    /// Tailnet HTTPS address, present once HTTPS is available and the device's
    /// name is known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}
