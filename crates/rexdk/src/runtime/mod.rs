mod api;
mod config;
mod connection;
mod error;
mod event_stream;

pub use api::events;
pub use config::{ClientConfig, Transport};
pub use error::Error;
pub use event_stream::EventStream;

use std::sync::{Arc, atomic::AtomicU64};

use futures_util::StreamExt;
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::mpsc;

use crate::runtime::connection::{Connection, ConnectionState};

const EVENT_BUFFER_CAPACITY: usize = 64;

#[derive(Debug)]
pub struct Client {
    config: ClientConfig,
    reconnect_token: Option<String>,
    connection: Option<connection::Connection>,
}

impl Client {
    pub fn new(config: Option<ClientConfig>) -> Self {
        Self {
            config: config.unwrap_or_default(),
            reconnect_token: None,
            connection: None,
        }
    }

    pub async fn connect(&mut self) -> Result<(), Error> {
        let mut control_stream = self.create_transport().await?;
        let welcome = self.handshake(&mut control_stream).await?;
        let (writer, reader) = control_stream.split();

        let (event_tx, event_rx) = mpsc::channel(EVENT_BUFFER_CAPACITY);
        let state = Arc::new(tokio::sync::Mutex::new(ConnectionState::default()));
        let ws_reader_worker = Connection::spawn_reader(reader, Arc::clone(&state), event_tx);

        self.reconnect_token = welcome.client.map(|client| client.reconnect_token);
        self.connection = Some(Connection {
            ws_stream_writer: tokio::sync::Mutex::new(writer),
            ws_reader_worker,
            event_stream: Some(EventStream::new(event_rx)),
            next_request_id: AtomicU64::new(1),
            state,
        });

        Ok(())
    }

    fn connection(&self) -> Result<&connection::Connection, Error> {
        self.connection.as_ref().ok_or(Error::NotConnected)
    }

    /// Takes the event stream from the connection.
    ///
    /// The stream can be moved into another task while the client keeps
    /// handling requests. It can only be taken once per connection.
    pub fn take_event_stream(&mut self) -> Result<EventStream, Error> {
        let connection = self.connection.as_mut().ok_or(Error::NotConnected)?;
        connection
            .event_stream
            .take()
            .ok_or(Error::EventStreamTaken)
    }

    pub async fn invoke<P, R>(&self, method: &str, params: Option<P>) -> Result<R, Error>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        self.connection()?.invoke(method, params).await
    }
}
