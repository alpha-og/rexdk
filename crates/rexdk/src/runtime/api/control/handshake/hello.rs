use serde::Serialize;

#[derive(Debug, Default, Serialize)]
pub struct Hello {
    r#type: String,
    api_revision: u32,
    api_min_revision: u32,
    client: HelloClientInfo,
}

#[derive(Debug, Default, Serialize)]
struct HelloClientInfo {
    kind: Option<ClientKind>,
    name: String,
    platform: Option<Platform>,
    version: String,
    label: String,
}

#[derive(Debug, Serialize)]
pub enum ClientKind {
    App,
    Cli,
    Agent,
    Other(String),
}

#[derive(Debug, Serialize)]
pub enum Platform {
    MacOS,
    IOS,
    Linux,
    Windows,
    FreeBSD,
    Web,
    Other(String),
}

impl Hello {
    pub fn new() -> Self {
        Self {
            r#type: "hello".into(),
            api_revision: 7,
            client: HelloClientInfo {
                kind: Some(ClientKind::Cli),
                name: "rexdk".into(),
                version: "0.1.0".into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}
