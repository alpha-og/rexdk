use serde::Deserialize;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Deserialize)]
pub struct Welcome {
    pub r#type: String,
    pub protocol: String,
    pub api_revision: u32,
    pub client: WelcomeClientInfo,
    pub limits: Limits,
}

#[derive(Debug, Deserialize)]
pub struct WelcomeClientInfo {
    pub id: String,
    pub reconnect_token: String,
}

#[derive(Debug, Deserialize)]
pub struct Limits {
    pub max_message_bytes: usize,
}

impl TryFrom<Message> for Welcome {
    type Error = anyhow::Error;

    fn try_from(value: Message) -> anyhow::Result<Self> {
        let Message::Text(text) = value else {
            anyhow::bail!("Expected WELCOME text message");
        };

        Ok(serde_json::from_str(&text)?)
    }
}
