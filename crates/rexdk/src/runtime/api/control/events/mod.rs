//! Rex control-connection event types and decoding.
//!
//! Server events arrive unwrapped. Session events arrive inside a
//! `session_event` wrapper that names the session; the normalized [`Event`]
//! carries that session ID alongside the typed payload.
//!
//! Decoding is deliberately total: an event name or payload that this SDK does
//! not recognize becomes [`Event::Unknown`] rather than an error, so a newer
//! server can add events without breaking the connection's event sequence.

mod client;
mod server;
mod session;

pub use client::{Client, ClientInfo, ClientReference, Principal};
use serde::Deserialize;
use serde_json::Value;
pub use server::{
    ClientActionError, ClientCallRequested, ClientChanged, ClientJoined, ClientLeft,
    ClientPerformRequested, ClientPerformed, ClientScriptEventsDropped, HostsChanged,
    KeymapChanged, ServerEvent, SessionCreated, SessionDestroyed, SessionMoved, SessionSummary,
    TailscaleAccount, TailscaleHttps, TailscaleStatusChanged,
};
pub use session::{
    ActiveWindowChanged, BlockClosed, BlockCreated, BlockOwnerChanged, ClientConnected,
    ClientConnectionChanged, ClientDisconnected, RawExtensionEvent, SessionEvent,
    SessionLabelChanged, SessionViewChanged, WindowClosed, WindowCreated, WindowLabelChanged,
};

/// A control-connection `event` message.
///
/// The server sends events without a request. `seq` starts at 1 and goes up by
/// one for each event on the connection, so a gap means an event was dropped.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EventMessage {
    /// Event's place in the connection's event sequence.
    pub seq: u64,
    /// The event, with its own `type` and `payload`.
    pub payload: Event,
}

/// A decoded Rex event.
///
/// Rex events have two protocol scopes: server and session. Client-related
/// notifications are server events, not a separate protocol scope. Creator
/// events are represented by [`SessionEvent::BlockEvent`].
#[derive(Debug, Clone)]
pub enum Event {
    /// An event emitted at server scope.
    Server(ServerEvent),

    /// An event emitted at session scope.
    Session {
        /// Session the event belongs to.
        session_id: String,
        event: SessionEvent,
    },

    /// An event whose name or payload is not recognized by this SDK.
    ///
    /// Preserve it so applications can inspect events introduced by a newer
    /// Rex API revision. `session_id` is set when the unknown event arrived
    /// inside a `session_event` wrapper.
    Unknown {
        session_id: Option<String>,
        raw: RawEvent,
    },
}

impl Event {
    /// Scope in which Rex emitted this event.
    pub fn scope(&self) -> EventScope {
        match self {
            Self::Server(_) => EventScope::Server,
            Self::Session { .. } => EventScope::Session,
            Self::Unknown { session_id, .. } => match session_id {
                Some(_) => EventScope::Session,
                None => EventScope::Server,
            },
        }
    }

    /// Session the event belongs to, if it is a session-scoped event.
    pub fn session_id(&self) -> Option<&str> {
        match self {
            Self::Server(_) => None,
            Self::Session { session_id, .. } => Some(session_id),
            Self::Unknown { session_id, .. } => session_id.as_deref(),
        }
    }

    fn from_value(value: Value) -> Self {
        let Some(name) = value.get("type").and_then(Value::as_str) else {
            return Self::Unknown {
                session_id: None,
                raw: raw_event(&value),
            };
        };

        // Session events are wrapped in `session_event`, whose payload names
        // the session and holds the inner event.
        if name == "session_event" {
            let Some(payload) = value.get("payload") else {
                return Self::Unknown {
                    session_id: None,
                    raw: raw_event(&value),
                };
            };

            let session_id = payload
                .get("session_id")
                .and_then(Value::as_str)
                .map(str::to_owned);

            let Some(inner) = payload.get("event") else {
                return Self::Unknown {
                    session_id,
                    raw: raw_event(&value),
                };
            };

            return match (SessionEvent::deserialize(inner), session_id) {
                (Ok(event), Some(session_id)) => Self::Session { session_id, event },
                (_, session_id) => Self::Unknown {
                    session_id,
                    raw: raw_event(inner),
                },
            };
        }

        // Every other event name is a server event.
        match ServerEvent::deserialize(&value) {
            Ok(event) => Self::Server(event),
            Err(_) => Self::Unknown {
                session_id: None,
                raw: raw_event(&value),
            },
        }
    }
}

impl<'de> Deserialize<'de> for Event {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Ok(Self::from_value(value))
    }
}

/// The protocol scope in which Rex emitted an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventScope {
    /// An event about the Rex server or its connected clients.
    Server,
    /// An event about a particular session.
    Session,
}

/// An event that this SDK version does not recognize.
#[derive(Debug, Clone, Deserialize)]
pub struct RawEvent {
    /// Rex event name.
    #[serde(rename = "type")]
    pub name: String,
    /// The event's inner payload. `null` is used if the message has none.
    #[serde(default)]
    pub payload: Value,
}

fn raw_event(value: &Value) -> RawEvent {
    RawEvent {
        name: value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        payload: value.get("payload").cloned().unwrap_or(Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::api::ControlMessage;

    fn decode(json: &str) -> EventMessage {
        match serde_json::from_str::<ControlMessage>(json).expect("message decodes") {
            ControlMessage::Event(event) => event,
            other => panic!("expected an event message, got {other:?}"),
        }
    }

    #[test]
    fn decodes_server_event() {
        let event = decode(
            r#"{"type":"event","seq":1,"payload":{"type":"session_created","payload":{"session":{"session_id":"session:1","label":"work"}}}}"#,
        );

        assert_eq!(event.seq, 1);
        assert_eq!(event.payload.scope(), EventScope::Server);
        assert_eq!(event.payload.session_id(), None);

        match event.payload {
            Event::Server(ServerEvent::SessionCreated(created)) => {
                assert_eq!(created.session.session_id, "session:1");
                assert_eq!(created.session.label, "work");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn decodes_session_event_with_session_id() {
        let event = decode(
            r#"{"type":"event","seq":2,"payload":{"type":"session_event","payload":{"session_id":"session:1","event":{"type":"block_created","payload":{"block_id":"block:1"}}}}}"#,
        );

        assert_eq!(event.payload.scope(), EventScope::Session);
        assert_eq!(event.payload.session_id(), Some("session:1"));

        match event.payload {
            Event::Session { session_id, event } => {
                assert_eq!(session_id, "session:1");
                match event {
                    SessionEvent::BlockCreated(block) => assert_eq!(block.block_id, "block:1"),
                    other => panic!("unexpected session event: {other:?}"),
                }
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn decodes_session_unit_event() {
        let event = decode(
            r#"{"type":"event","seq":3,"payload":{"type":"session_event","payload":{"session_id":"session:1","event":{"type":"creator_changed"}}}}"#,
        );

        match event.payload {
            Event::Session {
                event: SessionEvent::CreatorChanged,
                ..
            } => {}
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn decodes_block_event_payload() {
        let event = decode(
            r#"{"type":"event","seq":4,"payload":{"type":"session_event","payload":{"session_id":"session:1","event":{"type":"block_event","payload":{"block_id":"block:1","creator_name":"com.superlogical.terminal","name":"bell","payload":{"loud":true}}}}}}"#,
        );

        match event.payload {
            Event::Session {
                event: SessionEvent::BlockEvent(extension),
                ..
            } => {
                assert_eq!(extension.block_id, "block:1");
                assert_eq!(extension.name, "bell");
                assert_eq!(
                    extension.creator_name.as_deref(),
                    Some("com.superlogical.terminal")
                );
                assert_eq!(extension.payload, Some(serde_json::json!({"loud": true})));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn unknown_server_event_is_preserved() {
        let event = decode(
            r#"{"type":"event","seq":5,"payload":{"type":"future_server_event","payload":{"value":42}}}"#,
        );

        assert_eq!(event.payload.scope(), EventScope::Server);
        match event.payload {
            Event::Unknown { session_id, raw } => {
                assert_eq!(session_id, None);
                assert_eq!(raw.name, "future_server_event");
                assert_eq!(raw.payload, serde_json::json!({"value": 42}));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn unknown_session_event_keeps_session_id() {
        let event = decode(
            r#"{"type":"event","seq":6,"payload":{"type":"session_event","payload":{"session_id":"session:1","event":{"type":"future_session_event","payload":{"value":7}}}}}"#,
        );

        match event.payload {
            Event::Unknown { session_id, raw } => {
                assert_eq!(session_id.as_deref(), Some("session:1"));
                assert_eq!(raw.name, "future_session_event");
                assert_eq!(raw.payload, serde_json::json!({"value": 7}));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn malformed_known_event_is_preserved() {
        let event =
            decode(r#"{"type":"event","seq":7,"payload":{"type":"session_created","payload":{}}}"#);

        match event.payload {
            Event::Unknown { session_id, raw } => {
                assert_eq!(session_id, None);
                assert_eq!(raw.name, "session_created");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn decodes_tailscale_control_url() {
        let event = decode(
            r#"{"type":"event","seq":8,"payload":{"type":"tailscale_status_changed","payload":{"state":"running","allow_tailnet_members":true,"control_url":"https://headscale.example"}}}"#,
        );

        match event.payload {
            Event::Server(ServerEvent::TailscaleStatusChanged(status)) => {
                assert_eq!(
                    status.control_url.as_deref(),
                    Some("https://headscale.example")
                );
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
