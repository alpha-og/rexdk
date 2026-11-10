use serde_json::Value;

use crate::runtime::api::{
    control::{Hello, Request, Response, Welcome},
    events::EventMessage,
};

/// A control-connection message.
///
/// Control messages are JSON text on `/api/control`, tagged by `type`.
/// [`ControlMessage::Hello`] and [`ControlMessage::Request`] go from client to
/// server; the other variants come from the server. A `type` this SDK does not
/// know decodes as [`ControlMessage::Unknown`] so a newer server can add one
/// without breaking the connection.
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type")]
// Messages are handled one at a time while reading a frame; boxing the event
// payload would add an allocation on the hot path for no practical gain.
#[allow(clippy::large_enum_variant)]
pub enum ControlMessage {
    /// Client HELLO. Sent directly rather than parsed back.
    #[serde(rename = "hello", skip_deserializing)]
    #[allow(dead_code)]
    Hello(Hello),

    /// Server WELCOME, always its first message on the connection.
    #[serde(rename = "welcome")]
    Welcome(Welcome),

    /// Client request. Typed parameters are serialized through [`Request`]
    /// directly; this variant is the non-generic wire view.
    #[serde(rename = "request", skip_deserializing)]
    #[allow(dead_code)]
    Request(Request<Value>),

    /// Server response to a client request.
    #[serde(rename = "response")]
    Response(Response),

    /// Server event, sent without a request.
    #[serde(rename = "event")]
    Event(EventMessage),

    /// A `type` this SDK does not recognize.
    #[serde(other)]
    Unknown,
}
