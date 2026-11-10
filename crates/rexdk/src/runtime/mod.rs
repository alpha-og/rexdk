mod api;
mod config;
mod connection;
mod data_stream;
mod disconnect;
mod error;
mod event_stream;
mod transport;

pub use api::{data::decode_ticket, events};
pub use config::{ClientConfig, KeepAlive};
pub use connection::{
    ConnectionContext, ConnectionError, ConnectionErrorKind, ConnectionPhase, ControlError,
    DataError, DataStreamHandle,
};
pub use data_stream::DataStream;
pub use disconnect::DisconnectReason;
pub use error::Error;
pub use event_stream::EventStream;
pub use transport::{Transport, TransportError};

use std::sync::{Arc, atomic::AtomicU64};

use futures_util::{SinkExt, StreamExt};
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::{mpsc, watch};

use crate::{
    events::EventMessage,
    runtime::connection::{
        CONTROL_ENDPOINT, CONTROL_SUBPROTOCOL, ConnectionState, ControlConnection, DATA_ENDPOINT,
        DataConnectContext, connect as open_connection, control_handshake,
    },
};

const EVENT_BUFFER_CAPACITY: usize = 64;

#[derive(Debug)]
pub struct Client {
    config: ClientConfig,
    reconnect_token: Option<String>,
    resumed: bool,
    connection: Option<ControlConnection>,
    event_tx: mpsc::Sender<EventMessage>,
    event_rx: Option<mpsc::Receiver<EventMessage>>,
    disconnect_tx: watch::Sender<Option<DisconnectReason>>,
    disconnect_rx: watch::Receiver<Option<DisconnectReason>>,
    data_streams: Vec<DataStreamHandle>,
}

impl Client {
    pub fn new(config: Option<ClientConfig>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(EVENT_BUFFER_CAPACITY);
        let (disconnect_tx, disconnect_rx) = watch::channel(None);

        Self {
            config: config.unwrap_or_default(),
            reconnect_token: None,
            resumed: false,
            connection: None,
            event_tx,
            event_rx: Some(event_rx),
            disconnect_tx,
            disconnect_rx,
            data_streams: vec![],
        }
    }

    /// Opens a control connection and performs the `hello`/`welcome` handshake.
    ///
    /// If the client is already connected, the existing connection is dropped
    /// without a close frame first, so the new connection resumes the same
    /// Rex client when its token is still valid.
    pub async fn connect(&mut self) -> Result<(), Error> {
        let had_connection = self.connection.is_some();
        if had_connection {
            self.abort_connection(DisconnectReason::Local, false).await;
        }

        match self.connect_inner().await {
            Ok(()) => Ok(()),
            Err(err) => {
                if had_connection && self.disconnect_reason().is_none() {
                    self.disconnect_tx
                        .send_replace(Some(DisconnectReason::Local));
                }
                Err(err)
            }
        }
    }

    async fn connect_inner(&mut self) -> Result<(), Error> {
        let context = ConnectionContext::new(CONTROL_ENDPOINT, ConnectionPhase::Handshake);
        let mut control_stream = open_connection(
            &self.config.transport,
            CONTROL_ENDPOINT,
            CONTROL_SUBPROTOCOL,
        )
        .await
        .map_err(|source| {
            ConnectionError::new(
                context.clone(),
                ConnectionErrorKind::Control(ControlError::Transport(source)),
            )
        })?;

        let welcome = control_handshake(
            &self.config,
            self.reconnect_token.clone(),
            &mut control_stream,
            &context,
        )
        .await?;
        let max_message_bytes = welcome.limits.max_message_bytes;
        let (writer, reader) = control_stream.split();

        let stream_context = context.with_phase(ConnectionPhase::Stream);
        let state = Arc::new(tokio::sync::Mutex::new(ConnectionState::new(
            stream_context.clone(),
        )));
        let (pongs_tx, pongs_rx) = watch::channel(0u64);
        let writer = Arc::new(tokio::sync::Mutex::new(writer));

        // Reset before the reader starts, so an immediate close is not lost.
        self.disconnect_tx.send_replace(None);

        let reader_task = ControlConnection::spawn_reader(
            reader,
            Arc::clone(&state),
            self.event_tx.clone(),
            pongs_tx,
            self.disconnect_tx.clone(),
        );
        let keepalive_task = self.config.keepalive.clone().map(|config| {
            ControlConnection::spawn_keepalive(
                Arc::downgrade(&writer),
                pongs_rx,
                Arc::clone(&state),
                reader_task.abort_handle(),
                self.disconnect_tx.clone(),
                config,
            )
        });

        match welcome.client {
            Some(client) => {
                self.reconnect_token = Some(client.reconnect_token);
                self.resumed = client.resumed.unwrap_or(false);
            }
            None => {
                self.reconnect_token = None;
                self.resumed = false;
            }
        }

        self.connection = Some(ControlConnection {
            writer,
            reader_task,
            keepalive_task,
            next_request_id: AtomicU64::new(1),
            state,
            max_message_bytes,
            context: stream_context,
        });

        Ok(())
    }

    /// Drops the current connection without a WebSocket close frame.
    ///
    /// The reader and keepalive tasks are aborted, pending requests fail, and
    /// the socket is closed by dropping its writer.
    async fn abort_connection(&mut self, reason: DisconnectReason, publish: bool) {
        let Some(connection) = self.connection.take() else {
            return;
        };

        let ControlConnection {
            writer,
            reader_task,
            keepalive_task,
            state,
            ..
        } = connection;

        if let Some(task) = keepalive_task {
            task.abort();
        }
        reader_task.abort();

        ControlConnection::teardown(&state, &self.disconnect_tx, reason, publish).await;

        drop(writer);
    }

    /// Closes the connection with a WebSocket close frame.
    ///
    /// A close frame ends the client on the server at once, so the server
    /// invalidates the reconnect token. The token is kept in case a later
    /// connection reuses it, but the server will report `resumed: false` and
    /// start a new client. Use [`Client::reconnect`] to resume instead. Data
    /// connections are closed first. The event stream stays open; it ends when
    /// the `Client` is dropped.
    pub async fn disconnect(&mut self) -> Result<(), Error> {
        self.disconnect_data().await;

        let Some(connection) = self.connection.take() else {
            return Ok(());
        };

        let ControlConnection {
            writer,
            reader_task,
            keepalive_task,
            state,
            ..
        } = connection;

        if let Some(task) = keepalive_task {
            task.abort();
        }
        reader_task.abort();

        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), async {
            writer.lock().await.close().await
        })
        .await;

        ControlConnection::teardown(&state, &self.disconnect_tx, DisconnectReason::Local, true)
            .await;

        Ok(())
    }

    /// Reconnects using the reconnect token from the last welcome.
    ///
    /// Rex keeps a client for 60 seconds after its connection drops without a
    /// close frame. A reconnect inside that window resumes the same client and
    /// its session attachments; [`Client::is_resumed`] reports whether the
    /// server accepted the token. The old socket is dropped without a close
    /// frame, because a close frame would make the server forget the client at
    /// once.
    ///
    /// Events that happened while disconnected are lost and `seq` starts again
    /// at 1 on the new connection, so re-read state (`session.view` or
    /// `session.bootstrap`) before applying new events. Requests that were in
    /// flight fail with [`Error::Connection`]; the server may still have run
    /// them, so check state instead of blindly sending them again.
    pub async fn reconnect(&mut self) -> Result<(), Error> {
        let had_connection = self.connection.is_some() || self.disconnect_reason().is_some();
        self.abort_connection(DisconnectReason::Local, false).await;

        match self.connect_inner().await {
            Ok(()) => Ok(()),
            Err(err) => {
                // A concurrent wait_disconnected must not hang when the new
                // connection could not be opened.
                if had_connection && self.disconnect_reason().is_none() {
                    self.disconnect_tx
                        .send_replace(Some(DisconnectReason::Local));
                }
                Err(err)
            }
        }
    }

    /// Whether the last connection continued an existing Rex client.
    ///
    /// `false` means the server started a new client, so the caller must set
    /// the connection up again with `session.bootstrap` or `session.attach`.
    pub fn is_resumed(&self) -> bool {
        self.resumed
    }

    /// Whether the connection is currently open.
    ///
    /// This becomes `false` when the server closes the connection, when the
    /// keepalive times out, or after [`Client::disconnect`].
    pub fn is_connected(&self) -> bool {
        self.connection.is_some() && self.disconnect_rx.borrow().is_none()
    }

    /// Why the connection ended, if it has.
    ///
    /// The reason stays available after disconnection. A manual
    /// [`Client::reconnect`] clears it once the new connection is open.
    pub fn disconnect_reason(&self) -> Option<DisconnectReason> {
        self.disconnect_rx.borrow().clone()
    }

    /// Waits until the connection ends and returns why.
    ///
    /// This does not resolve for a successful manual [`Client::reconnect`],
    /// which replaces the connection in one call. It does resolve if the
    /// reconnect fails. Use it to stop consuming events or to start a
    /// reconnect loop:
    ///
    /// ```no_run
    /// # use rexdk::Client;
    /// # async fn example(mut client: Client) -> Result<(), rexdk::Error> {
    /// let mut events = client.take_event_stream()?;
    /// loop {
    ///     tokio::select! {
    ///         event = events.recv() => {
    ///             let Some(event) = event else { break };
    ///             println!("{event:?}");
    ///         }
    ///         reason = client.wait_disconnected() => {
    ///             println!("disconnected: {reason}");
    ///             break;
    ///         }
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn wait_disconnected(&self) -> DisconnectReason {
        let mut rx = self.disconnect_rx.clone();

        loop {
            if let Some(reason) = rx.borrow().clone() {
                return reason;
            }

            if rx.changed().await.is_err() {
                return self
                    .disconnect_rx
                    .borrow()
                    .clone()
                    .unwrap_or(DisconnectReason::Local);
            }
        }
    }

    /// Opens a data connection with a ticket from `data.connect`.
    ///
    /// The HELLO/WELCOME handshake is performed before this returns, so a
    /// refused ticket is reported here rather than on the stream. The returned
    /// handle can be used to disconnect; the client also keeps a clone so the
    /// connection is cleaned up when the client is dropped.
    pub async fn connect_data(
        &mut self,
        ticket: [u8; api::data::TICKET_LEN],
        subprotocol: &str,
    ) -> Result<(DataStream, DataStreamHandle), Error> {
        // Finished workers stay in the list until the next connection opens.
        self.data_streams.retain(|handle| !handle.is_finished());

        let context = DataConnectContext {
            transport: self.config.transport.clone(),
            subprotocol: subprotocol.to_owned(),
        };

        let (data_stream, handle) = DataStream::create(context, ticket).await?;
        self.data_streams.push(handle.clone());

        Ok((data_stream, handle))
    }

    /// Calls `data.connect` and opens the data connection in one step.
    ///
    /// `ConnectParams::subprotocol` must be set, because the WebSocket needs
    /// the same subprotocol in its handshake.
    pub async fn open_data(
        &mut self,
        params: crate::bindings::data::ConnectParams,
    ) -> Result<(DataStream, DataStreamHandle), Error> {
        let Some(subprotocol) = params.subprotocol.clone() else {
            let context = ConnectionContext::new(DATA_ENDPOINT, ConnectionPhase::Handshake);
            return Err(ConnectionError::new(
                context,
                ConnectionErrorKind::Data(DataError::MissingSubprotocol),
            )
            .into());
        };

        let output = crate::bindings::data::connect(self, params).await?;
        let ticket = decode_ticket(&output.ticket).map_err(|err| {
            ConnectionError::new(
                ConnectionContext::new(DATA_ENDPOINT, ConnectionPhase::Handshake),
                ConnectionErrorKind::Data(DataError::Protocol(err)),
            )
        })?;

        self.connect_data(ticket, &subprotocol).await
    }

    /// Closes every data connection gracefully.
    pub async fn disconnect_data(&mut self) {
        let handles = std::mem::take(&mut self.data_streams);

        futures_util::future::join_all(handles.iter().map(DataStreamHandle::disconnect)).await;
    }

    /// Takes the event stream from the connection.
    ///
    /// The stream can be moved into another task while the client keeps
    /// handling requests. It can only be taken once per `Client`, and it
    /// survives reconnects: it ends with `None` only when the `Client` is
    /// dropped. Use [`Client::wait_disconnected`] to observe a connection
    /// ending without dropping the client.
    pub fn take_event_stream(&mut self) -> Result<EventStream, Error> {
        self.event_rx
            .take()
            .map(EventStream::new)
            .ok_or(Error::EventStreamTaken)
    }

    pub async fn invoke<P, R>(&self, method: &str, params: Option<P>) -> Result<R, Error>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        self.connection()?.invoke(method, params).await
    }

    fn connection(&self) -> Result<&ControlConnection, Error> {
        self.connection.as_ref().ok_or_else(|| {
            let context = ConnectionContext::new(CONTROL_ENDPOINT, ConnectionPhase::Stream);
            self.disconnect_rx
                .borrow()
                .clone()
                .map_or(Error::NotConnected, |reason| reason.to_error(context))
        })
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        for handle in &self.data_streams {
            handle.abort();
        }

        if let Some(connection) = self.connection.take() {
            if let Some(task) = connection.keepalive_task {
                task.abort();
            }
            connection.reader_task.abort();
        }
    }
}
