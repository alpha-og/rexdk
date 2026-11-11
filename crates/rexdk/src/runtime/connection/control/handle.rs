use std::sync::{Arc, atomic::AtomicU64};

use futures_util::{SinkExt, StreamExt};
use serde::{Serialize, de::DeserializeOwned};
use tokio::{
    sync::{mpsc, watch},
    time::timeout,
};

use crate::runtime::config::ClientConfig;
use crate::runtime::connection::{
    ConnectionContext, ConnectionError, ConnectionErrorKind, ConnectionPhase, ControlError,
};
use crate::{DisconnectReason, Error};

use super::connection::{Connection, ConnectionState};
use super::handshake::handshake;
use super::{CONTROL_ENDPOINT, CONTROL_SUBPROTOCOL, EventChannel};

/// A handle to the control connection's lifecycle.
///
/// Like [`DataStreamHandle`](crate::DataStreamHandle), the handle outlives any
/// single socket. It owns the event and disconnect channels, the reconnect
/// token, and the negotiated revision, so those survive a drop and can be used
/// by [`ControlHandle::reconnect`]. The [`crate::Client`] owns the only handle.
#[derive(Debug)]
pub struct ControlHandle {
    inner: Arc<ControlHandleInner>,
}

#[derive(Debug)]
struct ControlHandleInner {
    connection: std::sync::Mutex<Option<Arc<Connection>>>,
    events: EventChannel,
    event_rx: std::sync::Mutex<Option<mpsc::Receiver<crate::events::EventMessage>>>,
    lagged_rx: watch::Receiver<Option<u64>>,
    disconnect_tx: watch::Sender<Option<DisconnectReason>>,
    disconnect_rx: watch::Receiver<Option<DisconnectReason>>,
    session: std::sync::Mutex<SessionState>,
}

/// State that outlives a single socket.
#[derive(Debug, Default)]
struct SessionState {
    reconnect_token: Option<String>,
    resumed: bool,
    api_revision: Option<u32>,
}

fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl ControlHandle {
    pub fn new(event_buffer_capacity: usize) -> Self {
        let capacity = event_buffer_capacity.max(1);
        let (event_tx, event_rx) = mpsc::channel(capacity);
        let (lagged_tx, lagged_rx) = watch::channel(None);
        let (disconnect_tx, disconnect_rx) = watch::channel(None);

        Self {
            inner: Arc::new(ControlHandleInner {
                connection: std::sync::Mutex::new(None),
                events: EventChannel {
                    events: event_tx,
                    lagged: lagged_tx,
                    capacity,
                },
                event_rx: std::sync::Mutex::new(Some(event_rx)),
                lagged_rx,
                disconnect_tx,
                disconnect_rx,
                session: std::sync::Mutex::new(SessionState::default()),
            }),
        }
    }

    /// Opens a control connection and performs the `hello`/`welcome` handshake.
    ///
    /// If a connection is already open, it is dropped without a close frame
    /// first, so the new connection resumes the same Rex client when its token
    /// is still valid.
    pub async fn connect(&self, config: &ClientConfig) -> Result<(), Error> {
        let had_connection = lock(&self.inner.connection).is_some();
        if had_connection {
            self.abort(DisconnectReason::Local, false).await;
        }

        match self.connect_inner(config).await {
            Ok(()) => Ok(()),
            Err(err) => {
                if had_connection && self.disconnect_reason().is_none() {
                    self.inner
                        .disconnect_tx
                        .send_replace(Some(DisconnectReason::Local));
                }
                Err(err)
            }
        }
    }

    async fn connect_inner(&self, config: &ClientConfig) -> Result<(), Error> {
        lock(&self.inner.session).api_revision = None;

        let context = ConnectionContext::new(CONTROL_ENDPOINT, ConnectionPhase::Handshake);
        let mut control_stream = crate::runtime::connection::connect(
            &config.transport,
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

        let reconnect_token = lock(&self.inner.session).reconnect_token.clone();
        let welcome = handshake(config, reconnect_token, &mut control_stream, &context).await?;
        let max_message_bytes = welcome.limits.max_message_bytes;
        let (writer, reader) = control_stream.split();

        let stream_context = context.with_phase(ConnectionPhase::Stream);
        let state = Arc::new(tokio::sync::Mutex::new(ConnectionState::new(
            stream_context.clone(),
        )));
        let (pongs_tx, pongs_rx) = watch::channel(0u64);
        let writer = Arc::new(tokio::sync::Mutex::new(writer));

        // Reset before the reader starts, so an immediate close is not lost.
        self.inner.disconnect_tx.send_replace(None);
        self.inner.events.reset_lag();

        let reader_task = Connection::spawn_reader(
            reader,
            Arc::clone(&state),
            self.inner.events.clone(),
            pongs_tx,
            self.inner.disconnect_tx.clone(),
            max_message_bytes,
        );
        let keepalive_task = config.keepalive.clone().map(|config| {
            Connection::spawn_keepalive(
                Arc::downgrade(&writer),
                pongs_rx,
                Arc::clone(&state),
                reader_task.abort_handle(),
                self.inner.disconnect_tx.clone(),
                config,
            )
        });

        {
            let mut session = lock(&self.inner.session);
            session.api_revision = Some(welcome.api_revision.unwrap_or(0));

            match welcome.client {
                Some(client) => {
                    session.reconnect_token = Some(client.reconnect_token);
                    session.resumed = client.resumed.unwrap_or(false);
                }
                None => {
                    session.reconnect_token = None;
                    session.resumed = false;
                }
            }
        }

        let connection = Connection {
            writer,
            reader_task,
            keepalive_task,
            next_request_id: AtomicU64::new(1),
            state,
            max_message_bytes,
            context: stream_context,
        };

        *lock(&self.inner.connection) = Some(Arc::new(connection));

        Ok(())
    }

    /// Drops the current connection without a WebSocket close frame.
    ///
    /// The reader and keepalive tasks are aborted, pending requests fail, and
    /// the socket is closed by dropping its writer.
    async fn abort(&self, reason: DisconnectReason, publish: bool) {
        let Some(connection) = lock(&self.inner.connection).take() else {
            return;
        };

        if let Some(task) = &connection.keepalive_task {
            task.abort();
        }
        connection.reader_task.abort();

        Connection::teardown(
            &connection.state,
            &self.inner.disconnect_tx,
            reason,
            publish,
        )
        .await;
    }

    /// Closes the connection with a WebSocket close frame.
    ///
    /// A close frame ends the client on the server at once, so the server
    /// invalidates the reconnect token. The token is kept in case a later
    /// connection reuses it, but the server will report `resumed: false` and
    /// start a new client. Use [`ControlHandle::reconnect`] to resume instead.
    /// The event stream stays open; it ends when the client is dropped.
    pub async fn disconnect(&self) -> Result<(), Error> {
        let Some(connection) = lock(&self.inner.connection).take() else {
            return Ok(());
        };

        if let Some(task) = &connection.keepalive_task {
            task.abort();
        }
        connection.reader_task.abort();

        let _ = timeout(std::time::Duration::from_secs(1), async {
            connection.writer.lock().await.close().await
        })
        .await;

        Connection::teardown(
            &connection.state,
            &self.inner.disconnect_tx,
            DisconnectReason::Local,
            true,
        )
        .await;

        Ok(())
    }

    /// Reconnects using the reconnect token from the last welcome.
    ///
    /// The old socket is dropped without a close frame, because a close frame
    /// would make the server forget the client at once.
    pub async fn reconnect(&self, config: &ClientConfig) -> Result<(), Error> {
        let had_connection =
            lock(&self.inner.connection).is_some() || self.disconnect_reason().is_some();
        self.abort(DisconnectReason::Local, false).await;

        match self.connect_inner(config).await {
            Ok(()) => Ok(()),
            Err(err) => {
                // A concurrent wait_disconnected must not hang when the new
                // connection could not be opened.
                if had_connection && self.disconnect_reason().is_none() {
                    self.inner
                        .disconnect_tx
                        .send_replace(Some(DisconnectReason::Local));
                }
                Err(err)
            }
        }
    }

    /// Whether the last connection continued an existing Rex client.
    pub fn is_resumed(&self) -> bool {
        lock(&self.inner.session).resumed
    }

    /// Whether the connection is currently open.
    pub fn is_connected(&self) -> bool {
        lock(&self.inner.connection).is_some() && self.inner.disconnect_rx.borrow().is_none()
    }

    /// Revision of the control API negotiated with the server.
    pub fn api_revision(&self) -> Option<u32> {
        lock(&self.inner.session).api_revision
    }

    /// Why the connection ended, if it has.
    pub fn disconnect_reason(&self) -> Option<DisconnectReason> {
        self.inner.disconnect_rx.borrow().clone()
    }

    /// Waits until the connection ends and returns why.
    pub async fn wait_disconnected(&self) -> DisconnectReason {
        let mut rx = self.inner.disconnect_rx.clone();

        loop {
            if let Some(reason) = rx.borrow().clone() {
                return reason;
            }

            if rx.changed().await.is_err() {
                return self
                    .inner
                    .disconnect_rx
                    .borrow()
                    .clone()
                    .unwrap_or(DisconnectReason::Local);
            }
        }
    }

    /// Takes the event stream from the connection.
    pub fn take_event_stream(&self) -> Result<crate::runtime::EventStream, Error> {
        lock(&self.inner.event_rx)
            .take()
            .map(|rx| crate::runtime::EventStream::new(rx, self.inner.lagged_rx.clone()))
            .ok_or(Error::EventStreamTaken)
    }

    /// Sequence number of the first event the consumer failed to receive.
    ///
    /// `None` means the event stream has not lagged since the current
    /// connection opened. A value is set when the configured backlog fills,
    /// immediately before the connection ends with
    /// [`DisconnectReason::EventBufferFull`].
    pub fn lagged_event_seq(&self) -> Option<u64> {
        self.inner.events.lagged()
    }

    pub async fn invoke<P, R>(&self, method: &str, params: Option<P>) -> Result<(R, u64), Error>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        self.connection()?.invoke(method, params).await
    }

    fn connection(&self) -> Result<Arc<Connection>, Error> {
        if let Some(connection) = lock(&self.inner.connection).clone() {
            return Ok(connection);
        }

        let context = ConnectionContext::new(CONTROL_ENDPOINT, ConnectionPhase::Stream);
        Err(self
            .inner
            .disconnect_rx
            .borrow()
            .clone()
            .map_or(Error::NotConnected, |reason| reason.to_error(context)))
    }
}

impl Drop for ControlHandleInner {
    fn drop(&mut self) {
        let connection = self
            .connection
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();

        if let Some(connection) = connection {
            if let Some(task) = &connection.keepalive_task {
                task.abort();
            }
            connection.reader_task.abort();
        }
    }
}
