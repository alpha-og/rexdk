use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use tokio::{
    sync::{mpsc, oneshot, oneshot::Sender, watch},
    task::{AbortHandle, JoinHandle},
};
use tokio_tungstenite::tungstenite;

use crate::protocol;
use crate::runtime::connection::{
    ConnectionContext, ConnectionError, ConnectionErrorKind, ControlError, websocket_error,
};
use crate::runtime::transport::Stream;
use crate::{DisconnectReason, Error, KeepAlive};

use super::ControlWriter;

/// State tied to one control socket.
pub(super) struct Connection {
    pub(super) writer: ControlWriter,
    pub(super) reader_task: JoinHandle<()>,
    pub(super) keepalive_task: Option<JoinHandle<()>>,
    pub(super) next_request_id: AtomicU64,
    pub(super) state: Arc<tokio::sync::Mutex<ConnectionState>>,
    /// Largest message the server accepts, from the welcome's `limits`.
    pub(super) max_message_bytes: usize,
    /// Identifies this control connection in errors.
    pub(super) context: ConnectionContext,
}

#[derive(Debug)]
pub(super) struct ConnectionState {
    pub(super) pending_requests: HashMap<String, Sender<Result<(Value, u64), Error>>>,
    pub(super) closed: Option<DisconnectReason>,
    /// Identifies this control connection in errors.
    pub(super) context: ConnectionContext,
}

impl ConnectionState {
    pub(super) fn new(context: ConnectionContext) -> Self {
        Self {
            pending_requests: HashMap::new(),
            closed: None,
            context,
        }
    }
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection")
            .field("reader_task", &self.reader_task)
            .field("keepalive_task", &self.keepalive_task)
            .field("next_request_id", &self.next_request_id)
            .field("max_message_bytes", &self.max_message_bytes)
            .field("context", &self.context)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Connection {
    /// Starts the task that reads messages from the control connection.
    ///
    /// The task is the only owner of the read half. When it exits it latches
    /// the disconnect reason in `state`, publishes it on `disconnect_tx`, and
    /// fails every request that was still waiting for a response.
    pub(super) fn spawn_reader(
        mut reader: SplitStream<Stream>,
        state: Arc<tokio::sync::Mutex<ConnectionState>>,
        event_tx: mpsc::Sender<crate::events::EventMessage>,
        pongs: watch::Sender<u64>,
        disconnect_tx: watch::Sender<Option<DisconnectReason>>,
        max_message_bytes: usize,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let reason =
                Self::read_loop(&mut reader, &state, &event_tx, &pongs, max_message_bytes).await;
            Self::teardown(&state, &disconnect_tx, reason, true).await;
        })
    }

    /// Starts the client-side keepalive loop.
    ///
    /// Rex pings every 120 seconds. The client pings on the same schedule so it
    /// notices a connection that is gone without a close frame. A ping is
    /// answered when the reader observes a pong; after `max_missed` consecutive
    /// unanswered pings the connection is failed with
    /// [`DisconnectReason::KeepAliveTimeout`].
    pub(super) fn spawn_keepalive(
        writer: std::sync::Weak<tokio::sync::Mutex<SplitSink<Stream, tungstenite::Message>>>,
        mut pongs: watch::Receiver<u64>,
        state: Arc<tokio::sync::Mutex<ConnectionState>>,
        reader: AbortHandle,
        disconnect_tx: watch::Sender<Option<DisconnectReason>>,
        config: KeepAlive,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut missed = 0;

            loop {
                tokio::time::sleep(config.interval).await;

                let Some(writer) = writer.upgrade() else {
                    break;
                };

                let before = *pongs.borrow_and_update();
                let nonce = before.wrapping_add(1).to_be_bytes();
                let ping = {
                    let mut writer = writer.lock().await;
                    writer
                        .send(tungstenite::Message::Ping(nonce.to_vec().into()))
                        .await
                };
                if ping.is_err() {
                    break;
                }

                let answered = tokio::time::timeout(config.timeout, async {
                    loop {
                        match pongs.changed().await {
                            Ok(()) if *pongs.borrow() != before => return Some(()),
                            Ok(()) => {}
                            // The reader is gone, so its teardown already ran.
                            Err(_) => return None,
                        }
                    }
                })
                .await;

                match answered {
                    Ok(Some(())) => {
                        missed = 0;
                        continue;
                    }
                    Ok(None) => break,
                    Err(_) => {}
                }

                missed += 1;
                if missed < config.max_missed {
                    continue;
                }

                Self::teardown(
                    &state,
                    &disconnect_tx,
                    DisconnectReason::KeepAliveTimeout,
                    true,
                )
                .await;
                reader.abort();

                let _ = tokio::time::timeout(std::time::Duration::from_secs(1), async {
                    writer.lock().await.close().await
                })
                .await;

                break;
            }
        })
    }

    /// Records that the connection ended and fails every pending request.
    ///
    /// The first caller wins: later calls are no-ops. `publish` controls
    /// whether [`crate::Client::wait_disconnected`] observes this reason.
    pub(super) async fn teardown(
        state: &tokio::sync::Mutex<ConnectionState>,
        disconnect_tx: &watch::Sender<Option<DisconnectReason>>,
        reason: DisconnectReason,
        publish: bool,
    ) {
        let (context, pending): (ConnectionContext, Vec<_>) = {
            let mut guard = state.lock().await;
            if guard.closed.is_some() {
                return;
            }
            guard.closed = Some(reason.clone());
            (
                guard.context.clone(),
                guard.pending_requests.drain().map(|(_, tx)| tx).collect(),
            )
        };

        if publish {
            disconnect_tx.send_replace(Some(reason.clone()));
        }

        for tx in pending {
            let _ = tx.send(Err(reason.to_error(context.clone())));
        }
    }

    async fn read_loop(
        reader: &mut SplitStream<Stream>,
        state: &tokio::sync::Mutex<ConnectionState>,
        event_tx: &mpsc::Sender<crate::events::EventMessage>,
        pongs: &watch::Sender<u64>,
        max_message_bytes: usize,
    ) -> DisconnectReason {
        loop {
            match reader.next().await {
                Some(Ok(tungstenite::Message::Text(text))) => {
                    if text.len() > max_message_bytes {
                        return DisconnectReason::MessageTooLarge {
                            size: text.len(),
                            limit: max_message_bytes,
                        };
                    }

                    let Ok(message) =
                        serde_json::from_str::<protocol::ControlMessage>(text.as_str())
                    else {
                        // A single malformed frame does not invalidate the
                        // connection; keep serving subsequent messages.
                        continue;
                    };
                    match message {
                        protocol::ControlMessage::Response(response) => {
                            let tx = state.lock().await.pending_requests.remove(&response.id);
                            if let Some(tx) = tx {
                                let seq = response.seq;
                                let result = match (response.payload, response.error) {
                                    (Some(payload), _) => Ok((payload, seq)),
                                    (None, Some(error)) => Err(error.into()),
                                    (None, None) => Err(Error::Protocol(format!(
                                        "response {} had neither payload nor error",
                                        response.id
                                    ))),
                                };
                                let _ = tx.send(result);
                            }
                        }
                        protocol::ControlMessage::Event(event) => {
                            // If the consumer dropped the event stream, stop
                            // forwarding events but keep serving responses.
                            let _ = event_tx.send(event).await;
                        }
                        protocol::ControlMessage::Hello(_)
                        | protocol::ControlMessage::Request(_)
                        | protocol::ControlMessage::Welcome(_)
                        | protocol::ControlMessage::Unknown => {}
                    }
                }
                Some(Ok(tungstenite::Message::Pong(_))) => {
                    let next = pongs.borrow().wrapping_add(1);
                    pongs.send_replace(next);
                }
                Some(Ok(tungstenite::Message::Close(frame))) => {
                    return match frame {
                        Some(frame) => DisconnectReason::Server {
                            code: u16::from(frame.code),
                            reason: frame.reason.to_string(),
                        },
                        None => DisconnectReason::Server {
                            code: 1005,
                            reason: String::new(),
                        },
                    };
                }
                // tungstenite answers pings automatically while reading.
                Some(Ok(_)) => {}
                None => return DisconnectReason::Abnormal,
                Some(Err(tungstenite::Error::Protocol(
                    tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
                ))) => return DisconnectReason::Abnormal,
                Some(Err(err)) => return DisconnectReason::ReadError(err.to_string()),
            }
        }
    }

    async fn send(&self, message: tungstenite::Message) -> Result<(), Error> {
        let size = match &message {
            tungstenite::Message::Text(text) => text.len(),
            tungstenite::Message::Binary(bytes)
            | tungstenite::Message::Ping(bytes)
            | tungstenite::Message::Pong(bytes) => bytes.len(),
            tungstenite::Message::Close(_) | tungstenite::Message::Frame(_) => 0,
        };
        if size > self.max_message_bytes {
            return Err(Error::MessageTooLarge {
                size,
                limit: self.max_message_bytes,
            });
        }

        self.writer
            .lock()
            .await
            .send(message)
            .await
            .map_err(|source| {
                Error::Connection(ConnectionError::new(
                    self.context.clone(),
                    ConnectionErrorKind::Control(ControlError::Transport(websocket_error(
                        &self.context,
                        source,
                    ))),
                ))
            })?;
        Ok(())
    }

    pub(super) async fn invoke<P, R>(
        &self,
        method: &str,
        params: Option<P>,
    ) -> Result<(R, u64), Error>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let id = self
            .next_request_id
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| Error::RequestIdExhausted)?
            .to_string();
        let req = protocol::control::Request::new(&id, method, params);
        let json = serde_json::to_string(&req)?;

        let (tx, rx) = oneshot::channel();

        {
            let mut state = self.state.lock().await;
            if let Some(reason) = &state.closed {
                return Err(reason.to_error(self.context.clone()));
            }
            state.pending_requests.insert(id.clone(), tx);
        }

        if let Err(err) = self.send(tungstenite::Message::Text(json.into())).await {
            self.state.lock().await.pending_requests.remove(&id);
            return Err(err);
        }

        let (payload, seq) = rx.await??;

        Ok((serde_json::from_value(payload)?, seq))
    }
}
