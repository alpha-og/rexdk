use serde::Serialize;

use crate::runtime::config::{ClientConfig, ClientKind, Platform};

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

#[derive(Debug, Default, Serialize)]
struct ClientInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<ClientKind>,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    platform: Option<Platform>,
    version: String,
    label: String,
}

impl Hello {
    pub fn new(reconnect_token: Option<String>, client_config: &ClientConfig) -> Self {
        let platform = match std::env::consts::OS {
            "macos" => Platform::MacOS,
            "ios" => Platform::IOS,
            "linux" => Platform::Linux,
            "windows" => Platform::Windows,
            "freebsd" => Platform::FreeBSD,
            "emscripten" | "wasm32" => Platform::Web,
            other => Platform::Other(other.to_string()),
        };
        Self {
            r#type: "hello".into(),
            api_revision: Some(client_config.api_revision),
            api_min_revision: Some(client_config.api_min_revision),
            client: Some(ClientInfo {
                kind: Some(client_config.kind.clone()),
                name: client_config.name.clone(),
                version: client_config.version.clone(),
                label: client_config.label.clone(),
                platform: Some(platform),
            }),
            reconnect_token,
        }
    }
}
