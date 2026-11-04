mod error;
mod hello;
mod welcome;

pub use error::HandshakeError;
use hello::Hello;
use welcome::Welcome;

use futures_util::{SinkExt, StreamExt};
use tokio::net::UnixStream;
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

use crate::runtime::{Client, Error, config::ClientConfig};

impl Client {
    pub async fn handshake(
        client_config: &ClientConfig,
        reconnect_token: Option<String>,
        ws_stream: &mut WebSocketStream<UnixStream>,
    ) -> Result<Welcome, Error> {
        let hello = Hello::new(reconnect_token, client_config);
        let json = serde_json::to_string(&hello)?;

        ws_stream.send(Message::Text(json.into())).await?;

        let message = ws_stream
            .next()
            .await
            .ok_or(HandshakeError::ConnectionClosed)??;

        Ok(message.try_into()?)
    }
}
