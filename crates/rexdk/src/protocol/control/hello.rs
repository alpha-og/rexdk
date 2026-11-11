use serde::{Deserialize, Serialize};

/// Size limit for a serialized HELLO, in bytes.
///
/// Rex requires the whole HELLO to stay under this limit uncompressed, so that
/// compression cannot leak the reconnect token.
pub const HELLO_MAX_BYTES: usize = 512;

/// What sort of program a client is.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientKind {
    App,
    #[default]
    Cli,
    Agent,
    Other,
}

/// What platform a client runs on.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    MacOS,
    IOS,
    Linux,
    Windows,
    FreeBSD,
    Web,
    Other,
}

/// A client-to-server control HELLO.
///
/// HELLO is the first message on `/api/control` and must be sent within 5
/// seconds of opening the WebSocket. A client sends exactly one, then only
/// requests.
///
/// `reconnect_token` continues an existing client; leave it out to start a
/// new one. The token is sensitive: never log it, and keep the whole hello
/// under 512 bytes uncompressed so compression cannot leak it.
#[derive(Debug, Default, Serialize)]
pub struct Hello {
    r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reconnect_token: Option<String>,
    client: Option<ClientInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_revision: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_min_revision: Option<u32>,
}

/// Description of the connecting program in a HELLO.
#[derive(Debug, Default, Serialize)]
pub struct ClientInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<ClientKind>,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    platform: Option<Platform>,
    version: String,
    label: String,
}

impl Hello {
    /// Builds a hello for a client and the control API revisions it speaks.
    pub fn new(
        reconnect_token: Option<String>,
        client: ClientInfo,
        api_revision: u32,
        api_min_revision: u32,
    ) -> Self {
        Self {
            r#type: "hello".into(),
            api_revision: Some(api_revision),
            api_min_revision: Some(api_min_revision),
            client: Some(client),
            reconnect_token,
        }
    }
}

impl ClientInfo {
    /// Describes the program opening the connection.
    pub fn new(
        kind: ClientKind,
        name: impl Into<String>,
        version: impl Into<String>,
        label: impl Into<String>,
        platform: Platform,
    ) -> Self {
        Self {
            kind: Some(kind),
            name: name.into(),
            platform: Some(platform),
            version: version.into(),
            label: label.into(),
        }
    }
}
