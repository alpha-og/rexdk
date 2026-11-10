use serde::Deserialize;

/// A server-to-client control WELCOME.
///
/// WELCOME is always the server's first message on `/api/control`. Check
/// `protocol` (it must be `rex.control.v1`) before sending anything else. A
/// server that cannot serve the client closes the connection instead of
/// sending one.
#[derive(Debug, Deserialize)]
pub struct Welcome {
    /// Protocol of the control connection, `rex.control.v1`.
    pub protocol: String,
    /// Limits that apply to this connection.
    pub limits: Limits,
    /// Other ways to reach the same server.
    pub carriers: Option<Vec<Carrier>>,
    /// Revision of the control API used by every later message.
    pub api_revision: Option<u32>,
    /// Oldest control API revision the server supports.
    pub api_min_revision: Option<u32>,
    /// Newest control API revision the server supports.
    pub api_server_revision: Option<u32>,
    /// The client this connection serves.
    pub client: Option<WelcomeClientInfo>,
    /// Optional behaviors the server supports.
    pub features: Option<Vec<String>>,
}

/// Another way to reach the same server.
#[derive(Debug, Deserialize)]
pub struct Carrier {
    /// Carrier name, such as `quic`. Ignore names you do not know.
    pub carrier: String,
    /// Port the carrier accepts connections on.
    pub port: u16,
    /// TLS application protocols the carrier accepts, most preferred first.
    pub alpn: Option<Vec<String>>,
}

/// The client a control connection serves.
#[derive(Debug, Deserialize)]
pub struct WelcomeClientInfo {
    /// Stable client ID; it survives reconnects.
    pub id: String,
    /// Token to send in a later hello to continue this client.
    ///
    /// Each welcome rotates the token and the old one stops working. The token
    /// is sensitive and must not be logged.
    pub reconnect_token: String,
    /// Whether this connection continued an existing client.
    pub resumed: Option<bool>,
}

/// Limits of a control connection.
#[derive(Debug, Deserialize)]
pub struct Limits {
    /// Largest message the server sends or accepts, in bytes.
    pub max_message_bytes: usize,
}
