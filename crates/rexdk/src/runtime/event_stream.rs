//! The stream of events from a Rex control connection.
//!
//! The stream is stable across reconnects: it ends with `None` only when the
//! [`Client`](crate::Client) is dropped. Use
//! [`Client::wait_disconnected`](crate::Client::wait_disconnected) to observe a
//! connection ending without dropping the client.

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::Stream;
use tokio::sync::mpsc::Receiver;

use crate::events::EventMessage;

/// A stream of events received on the control connection.
///
/// Events arrive in order with no gaps in their `seq`. Rex closes a connection
/// rather than skip an event, so a slow consumer applies backpressure to the
/// reader. Read events promptly.
#[derive(Debug)]
pub struct EventStream {
    rx: Receiver<EventMessage>,
}

impl EventStream {
    pub fn new(rx: Receiver<EventMessage>) -> Self {
        Self { rx }
    }

    /// Receives the next event, or `None` once the client is dropped.
    pub async fn recv(&mut self) -> Option<EventMessage> {
        self.rx.recv().await
    }
}

impl Stream for EventStream {
    type Item = EventMessage;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().rx.poll_recv(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Event, RawEvent};
    use futures_util::StreamExt;

    fn event(seq: u64) -> EventMessage {
        EventMessage {
            seq,
            payload: Event::Unknown {
                session_id: None,
                raw: RawEvent {
                    name: "test_event".into(),
                    payload: serde_json::Value::Null,
                },
            },
        }
    }

    #[tokio::test]
    async fn stream_yields_events_until_sender_drops() {
        let (tx, rx) = tokio::sync::mpsc::channel(2);
        let mut stream = EventStream::new(rx);

        tx.send(event(1)).await.unwrap();
        tx.send(event(2)).await.unwrap();
        drop(tx);

        assert_eq!(stream.next().await.map(|event| event.seq), Some(1));
        assert_eq!(stream.next().await.map(|event| event.seq), Some(2));
        assert!(stream.next().await.is_none());
    }
}
