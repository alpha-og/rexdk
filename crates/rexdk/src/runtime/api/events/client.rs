//! Shared client identity and metadata types used by server and session events.

use serde::{Deserialize, Serialize};

/// Information a client advertises about itself.
///
/// This metadata is intended for display. It is not an authentication identity.
/// Rex advises treating unknown `kind` and `platform` values as `other`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientInfo {
    /// Program kind: `app`, `cli`, `agent`, or `other`.
    ///
    /// Left out when the client did not provide it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,

    /// Platform: `macos`, `ios`, `linux`, `windows`, `freebsd`, `web`, or `other`.
    ///
    /// Left out when the client did not provide it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,

    /// Program name, such as `Rex.app` or `rex`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Program version as reported by the client.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Client label. Labels are not unique.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// The server-identified identity of a client connection.
///
/// `display` can be stale or shared. For identity comparisons, prefer stable
/// identity fields such as `uid`, `sid`, or `user_id`, when applicable.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Principal {
    /// Identification mechanism: `peer`, `windows_peer`, `tailscale`,
    /// `anonymous`, or a future value. Preserve unknown strings.
    pub kind: String,

    /// Initial transport: `unix`, `named_pipe`, `tcp`, `tailscale`, or a future
    /// value. Login servers pass this through unchanged.
    pub transport: String,

    /// Human-readable identity, such as `uid 501` or `alice@example.com`.
    /// It may be stale or shared.
    pub display: String,

    /// Network address. Omitted when there is no address, such as for Unix
    /// socket connections.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_addr: Option<String>,

    /// Unix user ID. Present for `peer` identities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<u32>,

    /// Unix group ID. Present for `peer` identities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gid: Option<u32>,

    /// Process ID at connection time, when known, for `peer` or `windows_peer`.
    ///
    /// Process IDs can be reused; use this field for debugging, not durable
    /// identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,

    /// Windows security identifier. Present for `windows_peer` identities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>,

    /// Tailscale user ID. Present for `tailscale` identities.
    ///
    /// Check `tags` before treating this identity as a person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    /// Tailscale login name, usually an email address. Display-only because it
    /// can change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login_name: Option<String>,

    /// Tailscale device name. Present for `tailscale` identities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,

    /// Tailscale device tags. Any tag indicates a machine identity.
    ///
    /// Rex may omit this field or send `null` when there are no tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// Route the client took to the server, for troubleshooting.
    ///
    /// Entries can include `tailscale`, `tailnet`, `tailnet-serve`, and
    /// `login-server`. Rex may omit this field or send `null`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<Vec<String>>,
}

/// A client as reported by server-level `client_changed` and `client_joined`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Client {
    /// Client ID. Stable across reconnects.
    pub client_id: String,

    /// Client-advertised metadata. Omitted if the client supplied none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<ClientInfo>,

    /// Server-identified connection principal.
    pub principal: Principal,

    /// RFC 3339 timestamp of the client's first connection.
    ///
    /// This timestamp does not change when the client reconnects.
    pub connected_at: String,

    /// Connection state: `connected` or `lost`.
    ///
    /// Rex says missing or unknown values should be treated as `connected`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_state: Option<String>,

    /// RFC 3339 timestamp when the server noticed connection loss.
    ///
    /// Set only while `connection_state` is `lost`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_lost_at: Option<String>,

    /// Sessions the client attached to with `session.attach` or
    /// `session.bootstrap`, sorted by ID. Omitted when there are none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_ids: Option<Vec<String>>,
}

/// A client reference included in `client_perform_requested` and
/// `client_performed` events.
///
/// The `info` field is client-supplied display metadata. Use `principal` for
/// authorization and identity decisions.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientReference {
    /// Client ID.
    pub client_id: String,

    /// Client-advertised metadata. Omitted if the client supplied none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<ClientInfo>,

    /// Server-identified principal.
    pub principal: Principal,
}
