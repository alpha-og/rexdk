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
