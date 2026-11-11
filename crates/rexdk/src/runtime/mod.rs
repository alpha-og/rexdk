mod config;
mod connection;
mod data_stream;
mod disconnect;
mod error;
mod event_stream;
mod transport;

pub use crate::protocol::{control::events, data::decode_ticket};
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

use serde::{Serialize, de::DeserializeOwned};

use crate::runtime::connection::{ControlHandle, DATA_ENDPOINT, DataConnectContext};

#[derive(Debug)]
pub struct Client {
    config: ClientConfig,
    control: ControlHandle,
    data_streams: Vec<DataStreamHandle>,
}

impl Client {
    pub fn new(config: Option<ClientConfig>) -> Self {
        let config = config.unwrap_or_default();
        let control = ControlHandle::new(config.event_buffer_capacity);

        Self {
            config,
            control,
            data_streams: vec![],
        }
    }

    /// Opens a control connection and performs the `hello`/`welcome` handshake.
    ///
    /// If the client is already connected, the existing connection is dropped
    /// without a close frame first, so the new connection resumes the same
    /// Rex client when its token is still valid.
    pub async fn connect(&mut self) -> Result<(), Error> {
        self.control.connect(&self.config).await
    }

    /// Drops the current connection without a WebSocket close frame and opens
    /// a new one using the reconnect token from the last welcome.
    ///
    /// Rex keeps a client for 60 seconds after its connection drops without a
    /// close frame. A reconnect inside that window resumes the same client and
    /// its session attachments; [`Client::is_resumed`] reports whether the
    /// server accepted the token.
    ///
    /// Events that happened while disconnected are lost and `seq` starts again
    /// at 1 on the new connection, so re-read state (`session.view` or
    /// `session.bootstrap`) before applying new events. Requests that were in
    /// flight fail with [`Error::Connection`]; the server may still have run
    /// them, so check state instead of blindly sending them again.
    pub async fn reconnect(&mut self) -> Result<(), Error> {
        self.control.reconnect(&self.config).await
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
        self.control.disconnect().await
    }

    /// Whether the last connection continued an existing Rex client.
    ///
    /// `false` means the server started a new client, so the caller must set
    /// the connection up again with `session.bootstrap` or `session.attach`.
    pub fn is_resumed(&self) -> bool {
        self.control.is_resumed()
    }

    /// Whether the connection is currently open.
    ///
    /// This becomes `false` when the server closes the connection, when the
    /// keepalive times out, or after [`Client::disconnect`].
    pub fn is_connected(&self) -> bool {
        self.control.is_connected()
    }

    /// Revision of the control API negotiated with the server.
    ///
    /// Returns the revision the server named in the welcome, so every message
    /// on the connection uses that revision's shapes. `None` until a handshake
    /// succeeds.
    pub fn api_revision(&self) -> Option<u32> {
        self.control.api_revision()
    }

    /// Why the connection ended, if it has.
    ///
    /// The reason stays available after disconnection. A manual
    /// [`Client::reconnect`] clears it once the new connection is open.
    pub fn disconnect_reason(&self) -> Option<DisconnectReason> {
        self.control.disconnect_reason()
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
        self.control.wait_disconnected().await
    }

    /// Opens a data connection with a ticket from `data.connect`.
    ///
    /// The HELLO/WELCOME handshake is performed before this returns, so a
    /// refused ticket is reported here rather than on the stream. The returned
    /// handle can be used to disconnect; the client also keeps a clone so the
    /// connection is cleaned up when the client is dropped.
    pub async fn connect_data(
        &mut self,
        ticket: [u8; crate::protocol::data::TICKET_LEN],
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

        let (output, _) = crate::bindings::data::connect(self, params).await?;
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
        self.control.take_event_stream()
    }

    /// Sequence number of the first event the consumer failed to receive.
    ///
    /// `None` means the event backlog has not overflowed since the current
    /// connection opened. A value is set immediately before the connection
    /// ends with [`DisconnectReason::EventBufferFull`]; reconnect and read the
    /// state again.
    pub fn lagged_event_seq(&self) -> Option<u64> {
        self.control.lagged_event_seq()
    }

    pub async fn invoke<P, R>(&self, method: &str, params: Option<P>) -> Result<(R, u64), Error>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        self.control.invoke(method, params).await
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        for handle in &self.data_streams {
            handle.abort();
        }
    }
}
