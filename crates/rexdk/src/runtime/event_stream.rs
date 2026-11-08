use tokio::sync::mpsc::Receiver;

use crate::events::EventMessage;

#[derive(Debug)]
pub struct EventStream {
    #[allow(dead_code)] // exposed once the event API is wired up
    rx: Receiver<EventMessage>,
}

#[allow(dead_code)] // exposed once the event API is wired up
impl EventStream {
    pub fn new(rx: Receiver<EventMessage>) -> Self {
        Self { rx }
    }
    pub async fn recv(&mut self) -> Option<EventMessage> {
        self.rx.recv().await
    }
}
