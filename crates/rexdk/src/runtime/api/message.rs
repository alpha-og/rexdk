use crate::runtime::api::{Response, control::Welcome, events::EventMessage};

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    #[serde(rename = "welcome")]
    Welcome(Welcome),

    #[serde(rename = "response")]
    Response(Response),

    #[serde(rename = "event")]
    Event(EventMessage),

    #[serde(other)]
    Unknown,
}
