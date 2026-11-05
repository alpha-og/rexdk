use serde::Deserialize;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Deserialize)]
pub struct Welcome {
    pub r#type: String,
    pub protocol: String,
    pub limits: Limits,
    pub carriers: Option<Vec<Carrier>>,
    pub api_revision: Option<u32>,
    pub api_min_revision: Option<u32>,
    pub api_server_revision: Option<u32>,
    pub client: Option<WelcomeClientInfo>,
    pub features: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct Carrier {
    pub carrier: String,
    pub port: u16,
    pub alpn: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct WelcomeClientInfo {
    pub id: String,
    pub reconnect_token: String,
    pub resumed: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct Limits {
    pub max_message_bytes: usize,
}
