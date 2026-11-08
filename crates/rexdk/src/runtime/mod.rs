mod api;
mod config;
mod error;
mod event_stream;

pub use api::events;
pub use config::ClientConfig;
pub use error::Error;

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use api::Request;
use error::ConnectionError;

use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use tokio::{
    net::UnixStream,
    sync::{
        mpsc,
        oneshot::{self, Sender},
    },
};
use tokio_tungstenite::{
    WebSocketStream, client_async,
    tungstenite::{client::IntoClientRequest, http::header::SEC_WEBSOCKET_PROTOCOL},
};

use crate::runtime::{api::Message, event_stream::EventStream};

const EVENT_BUFFER_CAPACITY: usize = 64;

#[derive(Debug)]
pub struct Client {
    config: ClientConfig,
    reconnect_token: Option<String>,
    ws_stream_writer: tokio::sync::Mutex<
        SplitSink<WebSocketStream<UnixStream>, tokio_tungstenite::tungstenite::protocol::Message>,
    >,
    ws_reader_worker: tokio::task::JoinHandle<()>,
    next_request_id: AtomicU64,
    event_stream: EventStream,
    pending_requests: Arc<Mutex<HashMap<String, Sender<Result<Value, Error>>>>>,
}

impl Client {
    pub async fn connect(client_config: Option<ClientConfig>) -> Result<Self, Error> {
        let client_config = client_config.map_or(ClientConfig::default(), |cfg| cfg);
        let socket_path = client_config
            .socket_path
            .clone()
            .or_else(|| dirs::data_dir().map(|data_dir| data_dir.join("rex/server.sock")))
            .ok_or(Error::SocketPathResolutionFailed)?;
        let unix_stream = UnixStream::connect(socket_path).await?;
        let mut request = "ws://localhost/api/control".into_client_request()?;
        request
            .headers_mut()
            .insert(SEC_WEBSOCKET_PROTOCOL, "rex.control.v1".parse()?);
        let (mut ws_stream, _response) = client_async(request, unix_stream).await?;

        let welcome = Self::handshake(&client_config, None, &mut ws_stream).await?;
        let (writer, mut reader) = ws_stream.split();

        let (event_tx, event_rx) = mpsc::channel(EVENT_BUFFER_CAPACITY);

        let pending_requests: Arc<Mutex<HashMap<String, Sender<Result<Value, Error>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending = Arc::clone(&pending_requests);

        let ws_reader_worker = tokio::spawn(async move {
            while let Some(message) = reader.next().await {
                let message: Message = match message {
                    Ok(tokio_tungstenite::tungstenite::protocol::Message::Text(text)) => {
                        match serde_json::from_slice(&text.as_bytes()) {
                            Ok(message) => message,
                            Err(err) => {
                                eprintln!("Invalid message: {err}");
                                continue;
                            }
                        }
                    }
                    Ok(_) => continue,
                    Err(err) => {
                        eprintln!("WebSocker error: {}", err.to_string());
                        if let Ok(mut pending) = pending.lock() {
                            for (_, tx) in pending.drain() {
                                let _ = tx.send(Err(Error::Connection(
                                    ConnectionError::ConnectionClosed,
                                )));
                            }
                        }
                        break;
                    }
                };
                match message {
                    Message::Response(response) => {
                        if let Ok(mut pending_requests) = pending.lock() {
                            if let Some(tx) = pending_requests.remove(&response.id) {
                                if let Some(payload) = response.payload {
                                    let _ = tx.send(Ok(payload));
                                } else if let Some(error) = response.error {
                                    let _ = tx.send(Err(error.into()));
                                }
                            }
                        } else {
                            eprintln!("Mutex poisoned");
                        }
                    }
                    Message::Event(event) => {
                        event_tx.send(event).await.unwrap(); // TODO: handle event stream errors
                        // gracefully
                    }
                    _ => continue, // other types of messages are ignored
                };
            }
        });

        let client = Self {
            config: client_config,
            reconnect_token: welcome
                .client
                .map_or(None, |client| Some(client.reconnect_token)),
            ws_stream_writer: tokio::sync::Mutex::new(writer),
            ws_reader_worker,
            event_stream: EventStream::new(event_rx),
            next_request_id: AtomicU64::new(1),
            pending_requests,
        };

        Ok(client)
    }

    async fn send(
        &self,
        message: tokio_tungstenite::tungstenite::protocol::Message,
    ) -> anyhow::Result<()> {
        let mut writer = self.ws_stream_writer.lock().await;

        writer.send(message).await?;

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
        let req = Request::new(&id, method, params);

        let (tx, rx) = oneshot::channel();

        let mut pending_requests = self
            .pending_requests
            .lock()
            .map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;

        let json = serde_json::to_string(&req)?;

        pending_requests.insert(id.clone(), tx);
        drop(pending_requests);

        self.send(tokio_tungstenite::tungstenite::protocol::Message::Text(
            json.into(),
        ))
        .await
        .map_err(|err| {
            if let Ok(mut pending_requests) = self.pending_requests.lock() {
                pending_requests.remove(&id);
            }
            err
        })?;

        let payload = rx.await??;

        Ok(serde_json::from_value(payload)?)
    }
}
