use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::Stream;
use tokio::sync::mpsc::Receiver;

use crate::events::EventMessage;

#[derive(Debug)]
pub struct EventStream {
    rx: Receiver<EventMessage>,
}

impl EventStream {
    pub fn new(rx: Receiver<EventMessage>) -> Self {
        Self { rx }
    }

    /// Receives the next event, or `None` once the connection is closed.
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
