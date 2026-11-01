mod error;
mod hello;
mod welcome;

pub use error::HandshakeError;

use futures_util::{SinkExt, StreamExt};
use tokio::net::UnixStream;
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

use crate::runtime::{
    RexClient, RexError,
    control::handshake::{hello::Hello, welcome::Welcome},
};

impl RexClient {
    pub async fn handshake(
        ws_stream: &mut WebSocketStream<UnixStream>,
    ) -> Result<Welcome, RexError> {
        let hello = Hello::new();
        let json = serde_json::to_string(&hello)?;

        ws_stream.send(Message::Text(json.into())).await?;

        let message = ws_stream
            .next()
            .await
            .ok_or(HandshakeError::ConnectionClosed)??;

        Ok(message.try_into()?)
    }
}
