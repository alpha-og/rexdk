use super::api;

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
    io::{AsyncRead, AsyncWrite},
    net::{TcpStream, UnixStream},
    sync::{
        mpsc,
        oneshot::{self, Sender},
    },
};
use tokio_tungstenite::{
    WebSocketStream, client_async,
    tungstenite::{self, client::IntoClientRequest, http::header::SEC_WEBSOCKET_PROTOCOL},
};

use crate::{
    Client, Error,
    runtime::{
        config::Transport::{NamedPipe, Tailscale, Tcp, UnixSocket},
        error::ConnectionError,
        event_stream::EventStream,
    },
};

const CONTROL_ENDPOINT: &str = "/api/control";
pub const CONTROL_SUBPROTOCOL: &str = "rex.control.v1";

pub trait AsyncStream: AsyncRead + AsyncWrite {}

impl<T> AsyncStream for T where T: AsyncRead + AsyncWrite {}

type BoxedStream = Box<dyn AsyncStream + Unpin + Send>;
pub type ControlStream = WebSocketStream<BoxedStream>;

pub struct Connection {
    pub ws_stream_writer: tokio::sync::Mutex<SplitSink<ControlStream, tungstenite::Message>>,
    pub ws_reader_worker: tokio::task::JoinHandle<Result<(), Error>>,
    pub next_request_id: AtomicU64,
    pub event_stream: Option<EventStream>,
    pub state: Arc<tokio::sync::Mutex<ConnectionState>>,
}

#[derive(Debug, Default)]
pub struct ConnectionState {
    pub pending_requests: HashMap<String, Sender<Result<Value, Error>>>,
    pub closed: Option<CloseReason>,
}

#[derive(Debug, Clone)]
pub enum CloseReason {
    Eof,
    Failed(String),
}

impl CloseReason {
    pub fn to_error(&self) -> Error {
        match self {
            Self::Eof => Error::Connection(ConnectionError::ConnectionClosed),
            Self::Failed(reason) => Error::Connection(ConnectionError::Closed(reason.clone())),
        }
    }
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection")
            .field("ws_reader_worker", &self.ws_reader_worker)
            .field("next_request_id", &self.next_request_id)
            .field("event_stream", &self.event_stream)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Connection {
    pub fn spawn_reader(
        mut reader: SplitStream<ControlStream>,
        state: Arc<tokio::sync::Mutex<ConnectionState>>,
        event_tx: mpsc::Sender<crate::events::EventMessage>,
    ) -> tokio::task::JoinHandle<Result<(), Error>> {
        tokio::spawn(async move {
            let outcome = Self::read_loop(&mut reader, &state, &event_tx).await;

            let reason = match &outcome {
                Ok(()) => CloseReason::Eof,
                Err(err) => CloseReason::Failed(err.to_string()),
            };

            let pending: Vec<_> = {
                let mut guard = state.lock().await;
                guard.closed = Some(reason.clone());
                guard.pending_requests.drain().map(|(_, tx)| tx).collect()
            };

            for tx in pending {
                let _ = tx.send(Err(reason.to_error()));
            }

            outcome
        })
    }

    async fn read_loop(
        reader: &mut SplitStream<ControlStream>,
        state: &tokio::sync::Mutex<ConnectionState>,
        event_tx: &mpsc::Sender<crate::events::EventMessage>,
    ) -> Result<(), Error> {
        loop {
            match reader.next().await {
                Some(Ok(tungstenite::Message::Text(text))) => {
                    let Ok(message) = serde_json::from_str::<api::Message>(text.as_str()) else {
                        // A single malformed frame does not invalidate the
                        // connection; keep serving subsequent messages.
                        continue;
                    };
                    match message {
                        api::Message::Response(response) => {
                            let tx = state.lock().await.pending_requests.remove(&response.id);
                            if let Some(tx) = tx {
                                let result = match (response.payload, response.error) {
                                    (Some(payload), _) => Ok(payload),
                                    (None, Some(error)) => Err(error.into()),
                                    (None, None) => Err(Error::Protocol(format!(
                                        "response {} had neither payload nor error",
                                        response.id
                                    ))),
                                };
                                let _ = tx.send(result);
                            }
                        }
                        api::Message::Event(event) => {
                            // If the consumer dropped the event stream, stop
                            // forwarding events but keep serving responses.
                            let _ = event_tx.send(event).await;
                        }
                        api::Message::Welcome(_) | api::Message::Unknown => {}
                    }
                }
                Some(Ok(tungstenite::Message::Close(_))) | None => return Ok(()),
                Some(Ok(_)) => {} // ping/pong/binary/frame
                Some(Err(err)) => {
                    return Err(Error::Connection(ConnectionError::WebSocket(err)));
                }
            }
        }
    }

    pub async fn send(&self, message: tungstenite::Message) -> Result<(), Error> {
        self.ws_stream_writer.lock().await.send(message).await?;
        Ok(())
    }

    pub async fn invoke<P, R>(&self, method: &str, params: Option<P>) -> Result<R, Error>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let id = self
            .next_request_id
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| Error::RequestIdExhausted)?
            .to_string();
        let req = api::Request::new(&id, method, params);
        let json = serde_json::to_string(&req)?;

        let (tx, rx) = oneshot::channel();

        {
            let mut state = self.state.lock().await;
            if let Some(reason) = &state.closed {
                return Err(reason.to_error());
            }
            state.pending_requests.insert(id.clone(), tx);
        }

        if let Err(err) = self.send(tungstenite::Message::Text(json.into())).await {
            self.state.lock().await.pending_requests.remove(&id);
            return Err(err);
        }

        let payload = rx.await??;

        Ok(serde_json::from_value(payload)?)
    }
}

impl Client {
    pub async fn create_transport(&self) -> Result<ControlStream, Error> {
        let (stream, uri): (BoxedStream, String) = match &self.config.transport {
            UnixSocket { path } => {
                #[cfg(unix)]
                {
                    let stream = Box::new(UnixStream::connect(path).await?);
                    let uri = format!("ws://localhost{CONTROL_ENDPOINT}");
                    (stream, uri)
                }

                #[cfg(not(unix))]
                {
                    use crate::runtime::error::ConnectionError::UnsupportedTransport;
                    return Err(Error::Connection(UnsupportedTransport {
                        transport: "UnixSocket".into(),
                        platform: "Windows".into(),
                    }));
                }
            }
            Tcp { host, port } => {
                let stream = Box::new(TcpStream::connect((host.as_str(), *port)).await?);
                (stream, format!("ws://{host}:{port}/api/control"))
            }
            Tailscale { hostname } => {
                let stream = Box::new(TcpStream::connect((hostname.as_str(), 443)).await?);
                (stream, format!("wss://{hostname}/api/control"))
            }

            NamedPipe { path } => {
                #[cfg(windows)]
                {
                    let stream = Box::new(ClientOptions::new().open(path)?);
                    (stream, "ws://localhost/api/control".to_owned())
                }

                #[cfg(not(windows))]
                {
                    use crate::runtime::error::ConnectionError::UnsupportedTransport;

                    let _ = path;
                    return Err(Error::Connection(UnsupportedTransport {
                        transport: "NamedPipe".into(),
                        platform: "Non Windows".into(),
                    }));
                }
            }
        };
        let mut request = uri.into_client_request()?;
        request
            .headers_mut()
            .insert(SEC_WEBSOCKET_PROTOCOL, CONTROL_SUBPROTOCOL.parse()?);
        let (ws, _) = client_async(request, stream).await?;
        Ok(ws)
    }
}
