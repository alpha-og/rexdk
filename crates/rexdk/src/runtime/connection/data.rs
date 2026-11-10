//! Runtime implementation of a Rex data connection.
//!
//! A data connection streams the output of a single block. The connection is
//! established using a single-use ticket obtained through the control
//! connection's `data.connect` method.
//!
//! The data WebSocket is server-to-client only. After the HELLO/WELCOME
//! handshake, its binary messages carry a continuous byte stream. WebSocket
//! message boundaries do not correspond to stream boundaries.
//!
//! A stream can outlive its WebSocket connection. If the transport drops,
//! the client resumes the stream with the reconnect token and the number of
//! stream bytes it has accepted for delivery. The worker task owns the socket
//! for the whole life of the stream and replaces it on every resume.
//!
//! See the Rex data-connection protocol documentation for handshake,
//! resumption, and stream semantics.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
    time::timeout,
};
use tokio_tungstenite::tungstenite::{
    self,
    protocol::{CloseFrame, frame::coding::CloseCode},
};

use crate::{
    Error,
    runtime::{
        api::{
            DataMessage,
            data::{Hello, RECONNECT_TOKEN_LEN, Resume, TICKET_LEN, Welcome},
        },
        data_stream::DataStream,
        transport::{Stream, Transport},
    },
};

use super::{
    ConnectionContext, ConnectionError, ConnectionErrorKind, ConnectionPhase, DataError,
    HANDSHAKE_TIMEOUT, connect, websocket_error,
};

pub const DATA_ENDPOINT: &str = "/api/data";
pub const DATA_BUFFER_CAPACITY: usize = 64;

/// Time allowed for the worker to send its closing frame.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);

/// Time allowed for a graceful disconnect to finish before aborting.
const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(2);

const RECONNECT_BACKOFF_MIN: Duration = Duration::from_millis(100);
const RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(2);

/// Everything a data worker needs to open a replacement connection.
#[derive(Debug, Clone)]
pub(crate) struct DataConnectContext {
    pub transport: Transport,
    pub subprotocol: String,
}

impl DataConnectContext {
    /// Builds the shared error context for this data connection.
    fn connection_context(&self, phase: ConnectionPhase) -> ConnectionContext {
        ConnectionContext::new(DATA_ENDPOINT, phase)
    }
}

/// A cloneable handle to a running data connection.
///
/// The handle is what the [`crate::Client`] stores and what a consumer can
/// keep to disconnect deliberately. Dropping the last handle aborts the
/// worker; [`Self::disconnect`] closes the connection gracefully instead.
#[derive(Debug, Clone)]
pub struct DataStreamHandle {
    inner: Arc<DataStreamHandleInner>,
}

#[derive(Debug)]
struct DataStreamHandleInner {
    connection: Mutex<Option<Connection>>,
}

#[derive(Debug)]
struct Connection {
    worker: JoinHandle<()>,
    shutdown: watch::Sender<bool>,
}

impl DataStream {
    /// Opens a data connection and starts its worker.
    ///
    /// The transport and the HELLO/WELCOME handshake are both completed before
    /// this returns, so a refusal is reported as an error from the call rather
    /// than as a stream item.
    pub(crate) async fn create(
        context: DataConnectContext,
        ticket: [u8; TICKET_LEN],
    ) -> Result<(Self, DataStreamHandle), Error> {
        let handshake_context = context.connection_context(ConnectionPhase::Handshake);
        let mut stream = connect(&context.transport, DATA_ENDPOINT, &context.subprotocol)
            .await
            .map_err(|source| {
                ConnectionError::new(
                    handshake_context.clone(),
                    ConnectionErrorKind::Data(DataError::Transport(source)),
                )
            })?;
        let welcome = handshake(&mut stream, Hello::new(ticket), &handshake_context).await?;

        let (data_tx, data_rx) = mpsc::channel(DATA_BUFFER_CAPACITY);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let consumed_bytes = Arc::new(AtomicU64::new(0));
        let received_bytes = Arc::new(AtomicU64::new(0));
        let channel_nonce = Arc::new(AtomicU64::new(welcome.channel_nonce));
        let resume_timeout = Duration::from_millis(u64::from(welcome.resume_timeout_ms));

        let worker = Worker {
            context,
            data_tx,
            shutdown: shutdown_rx,
            received_bytes: Arc::clone(&received_bytes),
            channel_nonce: Arc::clone(&channel_nonce),
            reconnect_token: welcome.reconnect_token,
            resume_timeout,
        };

        let worker = tokio::spawn(worker.run(stream));

        let handle = DataStreamHandle {
            inner: Arc::new(DataStreamHandleInner {
                connection: Mutex::new(Some(Connection {
                    worker,
                    shutdown: shutdown_tx.clone(),
                })),
            }),
        };

        let data_stream = DataStream::new(
            data_rx,
            consumed_bytes,
            received_bytes,
            channel_nonce,
            resume_timeout,
            shutdown_tx,
        );

        Ok((data_stream, handle))
    }
}

impl DataStreamHandle {
    /// Whether the worker has finished, so the handle can be dropped.
    pub fn is_finished(&self) -> bool {
        let guard = self
            .inner
            .connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.as_ref().is_none_or(|c| c.worker.is_finished())
    }

    /// Closes the data connection with a normal WebSocket close frame.
    ///
    /// The server ends the stream when it receives status 1000. This waits up
    /// to one second for the worker to finish, then aborts it.
    pub async fn disconnect(&self) -> Result<(), Error> {
        let Some(mut connection) = self.take_connection() else {
            return Ok(());
        };

        connection.shutdown.send_replace(true);

        if timeout(DISCONNECT_TIMEOUT, &mut connection.worker)
            .await
            .is_err()
        {
            connection.worker.abort();
        }

        Ok(())
    }

    /// Stops the worker without waiting for a close frame.
    ///
    /// Used when the owning [`crate::Client`] is dropped. Prefer
    /// [`Self::disconnect`] when a graceful close matters.
    pub(crate) fn abort(&self) {
        if let Some(connection) = self.take_connection() {
            connection.shutdown.send_replace(true);
            connection.worker.abort();
        }
    }

    fn take_connection(&self) -> Option<Connection> {
        self.inner
            .connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

impl Drop for DataStreamHandleInner {
    fn drop(&mut self) {
        if let Ok(connection) = self.connection.get_mut()
            && let Some(connection) = connection.take()
        {
            connection.shutdown.send_replace(true);
            connection.worker.abort();
        }
    }
}

/// Sends HELLO and waits for WELCOME or REJECT.
async fn handshake(
    stream: &mut Stream,
    hello: Hello,
    context: &ConnectionContext,
) -> Result<Welcome, Error> {
    let message = timeout(HANDSHAKE_TIMEOUT, async {
        stream
            .send(tungstenite::Message::Binary(Bytes::from(hello)))
            .await
            .map_err(|source| {
                ConnectionError::new(
                    context.clone(),
                    ConnectionErrorKind::Data(DataError::Transport(websocket_error(
                        context, source,
                    ))),
                )
            })?;

        match stream.next().await {
            Some(Ok(message)) => Ok::<_, ConnectionError>(message),
            Some(Err(source)) => Err(ConnectionError::new(
                context.clone(),
                ConnectionErrorKind::Data(DataError::Transport(websocket_error(context, source))),
            )),
            None => Err(ConnectionError::new(
                context.clone(),
                ConnectionErrorKind::Data(DataError::Handshake("connection closed".into())),
            )),
        }
    })
    .await
    .map_err(|_| {
        ConnectionError::new(
            context.clone(),
            ConnectionErrorKind::Data(DataError::Timeout {
                seconds: HANDSHAKE_TIMEOUT.as_secs(),
            }),
        )
    })??;

    match message {
        tungstenite::Message::Binary(bytes) => {
            let message = DataMessage::try_from(bytes).map_err(|err| {
                ConnectionError::new(
                    context.clone(),
                    ConnectionErrorKind::Data(DataError::Protocol(err)),
                )
            })?;

            match message {
                DataMessage::Welcome(welcome) => Ok(welcome),
                DataMessage::Reject(reject) => Err(ConnectionError::new(
                    context.clone(),
                    ConnectionErrorKind::Data(DataError::Rejected(reject.reason())),
                )
                .into()),
                _ => Err(ConnectionError::new(
                    context.clone(),
                    ConnectionErrorKind::Data(DataError::Handshake(
                        "unexpected handshake message".into(),
                    )),
                )
                .into()),
            }
        }
        tungstenite::Message::Close(frame) => Err(ConnectionError::new(
            context.clone(),
            ConnectionErrorKind::Data(DataError::Handshake(close_description(frame))),
        )
        .into()),
        _ => Err(ConnectionError::new(
            context.clone(),
            ConnectionErrorKind::Data(DataError::Handshake(
                "unexpected message during handshake".into(),
            )),
        )
        .into()),
    }
}

/// The long-lived task that owns the data socket and resumes it.
#[derive(Debug)]
struct Worker {
    context: DataConnectContext,
    data_tx: mpsc::Sender<Result<Bytes, Error>>,
    shutdown: watch::Receiver<bool>,
    received_bytes: Arc<AtomicU64>,
    channel_nonce: Arc<AtomicU64>,
    reconnect_token: [u8; RECONNECT_TOKEN_LEN],
    resume_timeout: Duration,
}

impl Worker {
    async fn run(mut self, mut stream: Stream) {
        loop {
            match self.read_stream(&mut stream).await {
                StreamEnd::Shutdown => {
                    close_stream(stream).await;
                    return;
                }
                StreamEnd::StreamEnded => return,
                StreamEnd::Fatal(err) => {
                    let _ = self.data_tx.send(Err(err)).await;
                    return;
                }
                StreamEnd::Resume => {}
            }

            match self.resume().await {
                ResumeOutcome::Stream(resumed) => stream = resumed,
                ResumeOutcome::Shutdown => return,
                ResumeOutcome::Failed(err) => {
                    let _ = self.data_tx.send(Err(err)).await;
                    return;
                }
            }
        }
    }

    /// Reads stream bytes until the connection ends or should be resumed.
    async fn read_stream(&mut self, stream: &mut Stream) -> StreamEnd {
        let data_tx = self.data_tx.clone();
        let received_bytes = Arc::clone(&self.received_bytes);

        loop {
            if *self.shutdown.borrow() {
                return StreamEnd::Shutdown;
            }

            let shutdown = &mut self.shutdown;

            tokio::select! {
                biased;
                _ = shutdown.changed() => return StreamEnd::Shutdown,
                message = stream.next() => match message {
                    Some(Ok(tungstenite::Message::Binary(bytes))) => {
                        // The protocol promises 1 to 65,536 bytes per message.
                        if bytes.is_empty() {
                            continue;
                        }

                        let len = bytes.len() as u64;
                        let accepted = tokio::select! {
                            biased;
                            _ = shutdown.changed() => false,
                            result = data_tx.send(Ok(bytes)) => result.is_ok(),
                        };
                        if !accepted {
                            return StreamEnd::Shutdown;
                        }

                        // Count only bytes that reached the channel. Bytes
                        // still queued there survive a resume, so resuming at
                        // this offset neither loses nor duplicates them.
                        received_bytes.fetch_add(len, Ordering::Relaxed);
                    }
                    Some(Ok(tungstenite::Message::Close(frame))) => {
                        let context = self.context.connection_context(ConnectionPhase::Stream);
                        return classify_close(frame, &context);
                    }
                    // Tungstenite answers pings automatically while reading.
                    Some(Ok(_)) => {}
                    None => return StreamEnd::Resume,
                    Some(Err(tungstenite::Error::Protocol(
                        tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
                    ))) => return StreamEnd::Resume,
                    Some(Err(_)) => return StreamEnd::Resume,
                }
            }
        }
    }

    /// Reconnects and resumes the stream until the retention window closes.
    async fn resume(&mut self) -> ResumeOutcome {
        let context = self.context.connection_context(ConnectionPhase::Resume);
        let deadline = Instant::now() + self.resume_timeout;
        let mut delay = RECONNECT_BACKOFF_MIN;
        let mut attempted = false;

        loop {
            if *self.shutdown.borrow() {
                return ResumeOutcome::Shutdown;
            }

            if attempted && Instant::now() >= deadline {
                return ResumeOutcome::Failed(
                    ConnectionError::new(
                        context.clone(),
                        ConnectionErrorKind::Data(DataError::ResumeWindowElapsed),
                    )
                    .into(),
                );
            }
            attempted = true;

            match self.connect_and_resume().await {
                ResumeAttempt::Stream(stream) => return ResumeOutcome::Stream(stream),
                ResumeAttempt::Shutdown => return ResumeOutcome::Shutdown,
                ResumeAttempt::Failed(err) => return ResumeOutcome::Failed(err),
                ResumeAttempt::Retry => {}
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return ResumeOutcome::Failed(
                    ConnectionError::new(
                        context.clone(),
                        ConnectionErrorKind::Data(DataError::ResumeWindowElapsed),
                    )
                    .into(),
                );
            }

            if !self.wait_or_shutdown(delay.min(remaining)).await {
                return ResumeOutcome::Shutdown;
            }

            delay = (delay * 2).min(RECONNECT_BACKOFF_MAX);
        }
    }

    /// Opens one replacement transport and sends RESUME.
    async fn connect_and_resume(&mut self) -> ResumeAttempt {
        let transport = self.context.transport.clone();
        let subprotocol = self.context.subprotocol.clone();

        let stream = tokio::select! {
            biased;
            _ = self.shutdown.changed() => return ResumeAttempt::Shutdown,
            result = connect(&transport, DATA_ENDPOINT, &subprotocol) => match result {
                Ok(stream) => stream,
                Err(_) => return ResumeAttempt::Retry,
            },
        };

        self.resume_handshake(stream).await
    }

    async fn resume_handshake(&mut self, mut stream: Stream) -> ResumeAttempt {
        let context = self.context.connection_context(ConnectionPhase::Resume);
        let token = self.reconnect_token;
        let received = self.received_bytes.load(Ordering::Relaxed);
        let resume = Resume::new(token, received);

        let result = tokio::select! {
            biased;
            _ = self.shutdown.changed() => return ResumeAttempt::Shutdown,
            result = timeout(HANDSHAKE_TIMEOUT, async {
                stream
                    .send(tungstenite::Message::Binary(Bytes::from(resume)))
                    .await
                    .map_err(|source| {
                        ConnectionError::new(
                            context.clone(),
                            ConnectionErrorKind::Data(DataError::Transport(websocket_error(
                                &context, source,
                            ))),
                        )
                    })?;

                match stream.next().await {
                    Some(Ok(message)) => Ok::<_, Error>(message),
                    Some(Err(source)) => Err(ConnectionError::new(
                        context.clone(),
                        ConnectionErrorKind::Data(DataError::Transport(websocket_error(
                            &context, source,
                        ))),
                    )
                    .into()),
                    None => Err(ConnectionError::new(
                        context.clone(),
                        ConnectionErrorKind::Data(DataError::Handshake(
                            "connection closed".into(),
                        )),
                    )
                    .into()),
                }
            }) => result,
        };

        let message = match result {
            Ok(Ok(message)) => message,
            // A transport failure or timeout is transient; try again.
            Ok(Err(_)) | Err(_) => return ResumeAttempt::Retry,
        };

        match message {
            tungstenite::Message::Binary(bytes) => match DataMessage::try_from(bytes) {
                Ok(DataMessage::Welcome(welcome)) => {
                    self.reconnect_token = welcome.reconnect_token;
                    self.channel_nonce
                        .store(welcome.channel_nonce, Ordering::Relaxed);
                    ResumeAttempt::Stream(stream)
                }
                Ok(DataMessage::Reject(reject)) => ResumeAttempt::Failed(
                    ConnectionError::new(
                        context.clone(),
                        ConnectionErrorKind::Data(DataError::Rejected(reject.reason())),
                    )
                    .into(),
                ),
                Ok(_) => ResumeAttempt::Failed(
                    ConnectionError::new(
                        context.clone(),
                        ConnectionErrorKind::Data(DataError::Handshake(
                            "unexpected handshake message".into(),
                        )),
                    )
                    .into(),
                ),
                Err(err) => ResumeAttempt::Failed(
                    ConnectionError::new(
                        context.clone(),
                        ConnectionErrorKind::Data(DataError::Protocol(err)),
                    )
                    .into(),
                ),
            },
            tungstenite::Message::Close(_) | tungstenite::Message::Ping(_) => ResumeAttempt::Retry,
            _ => ResumeAttempt::Failed(
                ConnectionError::new(
                    context.clone(),
                    ConnectionErrorKind::Data(DataError::Handshake(
                        "unexpected message during handshake".into(),
                    )),
                )
                .into(),
            ),
        }
    }

    /// Waits for `duration` or a shutdown request. Returns false on shutdown.
    async fn wait_or_shutdown(&mut self, duration: Duration) -> bool {
        tokio::select! {
            biased;
            _ = self.shutdown.changed() => false,
            _ = tokio::time::sleep(duration) => true,
        }
    }
}

#[derive(Debug)]
enum StreamEnd {
    /// The consumer asked to close, or dropped the stream.
    Shutdown,
    /// The server ended the stream normally.
    StreamEnded,
    /// The stream cannot be resumed.
    Fatal(Error),
    /// The transport ended; try to resume.
    Resume,
}

#[allow(clippy::large_enum_variant)]
enum ResumeOutcome {
    Stream(Stream),
    Shutdown,
    Failed(Error),
}

#[allow(clippy::large_enum_variant)]
enum ResumeAttempt {
    Stream(Stream),
    Retry,
    Shutdown,
    Failed(Error),
}

fn classify_close(frame: Option<CloseFrame>, context: &ConnectionContext) -> StreamEnd {
    let Some(frame) = frame else {
        return StreamEnd::Resume;
    };

    let code = u16::from(frame.code);
    let reason = frame.reason.to_string();

    match code {
        // The stream ended, or the block closed and must not be reconnected.
        1000 | 4000 => StreamEnd::StreamEnded,
        // The server requires a fresh ticket and HELLO.
        1013 => StreamEnd::Fatal(
            ConnectionError::new(
                context.clone(),
                ConnectionErrorKind::Data(DataError::ReconnectRequired { code, reason }),
            )
            .into(),
        ),
        // These cannot be resumed.
        1008 | 1011 => StreamEnd::Fatal(
            ConnectionError::new(
                context.clone(),
                ConnectionErrorKind::Data(DataError::Closed { code, reason }),
            )
            .into(),
        ),
        // Keepalive timeouts, provider cancellations, and anything unknown
        // leave the stream waiting for a resume.
        _ => StreamEnd::Resume,
    }
}

async fn close_stream(mut stream: Stream) {
    let frame = CloseFrame {
        code: CloseCode::Normal,
        reason: "".into(),
    };

    let _ = timeout(CLOSE_TIMEOUT, stream.close(Some(frame))).await;
}

fn close_description(frame: Option<CloseFrame>) -> String {
    match frame {
        Some(frame) => format!(
            "server closed the handshake with status {}: {}",
            u16::from(frame.code),
            frame.reason
        ),
        None => "server closed the handshake without a status".into(),
    }
}
