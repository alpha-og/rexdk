mod error;
mod hello;
mod welcome;

use std::time::Duration;

pub use error::HandshakeError;
use hello::Hello;
pub use welcome::Welcome;

use futures_util::{SinkExt, StreamExt};
use tokio::{net::UnixStream, time::timeout};
use tokio_tungstenite::{WebSocketStream, tungstenite};

use crate::runtime::{Client, Error, api::Message, config::ClientConfig};

impl Client {
    pub async fn handshake(
        client_config: &ClientConfig,
        reconnect_token: Option<String>,
        ws_stream: &mut WebSocketStream<UnixStream>,
    ) -> Result<Welcome, Error> {
        let hello = Hello::new(reconnect_token, client_config);
        let json = serde_json::to_string(&hello)?;

        let ws_message = timeout(Duration::from_secs(5), async {
            ws_stream
                .send(tungstenite::Message::Text(json.into()))
                .await?;

            ws_stream
                .next()
                .await
                .ok_or(HandshakeError::ConnectionClosed)?
                .map_err(HandshakeError::from)
        })
        .await
        .map_err(|err| HandshakeError::Timeout(err.to_string()))??;

        let message: Message = match ws_message {
            tungstenite::Message::Text(text_bytes) => serde_json::from_slice(text_bytes.as_bytes())
                .map_err(|err| HandshakeError::InvalidWelcome(err.to_string()))?,
            _ => return Err(HandshakeError::UnexpectedMessage.into()),
        };

        let welcome = match message {
            Message::Welcome(welcome) => welcome,
            _ => return Err(HandshakeError::UnexpectedMessage.into()),
        };

        if welcome.protocol != "rex.control.v1" {
            return Err(HandshakeError::InvalidWelcome(format!(
                "expected protocol rex.control.v1, got {}",
                welcome.protocol
            ))
            .into());
        }

        Ok(welcome)
    }
}
