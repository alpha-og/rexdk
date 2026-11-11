use futures_util::{SinkExt, StreamExt};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite;

use crate::protocol;
use crate::protocol::control::{ClientInfo, HELLO_MAX_BYTES, Hello, Platform, Welcome};
use crate::runtime::config::ClientConfig;
use crate::runtime::connection::{
    ConnectionContext, ConnectionError, ConnectionErrorKind, ControlError, HANDSHAKE_TIMEOUT,
    websocket_error,
};
use crate::runtime::transport::Stream;

use super::CONTROL_SUBPROTOCOL;

/// Builds the HELLO describing this client.
fn hello(reconnect_token: Option<String>, config: &ClientConfig) -> Hello {
    Hello::new(
        reconnect_token,
        ClientInfo::new(
            config.kind.clone(),
            config.name.clone(),
            config.version.clone(),
            config.label.clone(),
            current_platform(),
        ),
        config.api_revision,
        config.api_min_revision,
    )
}

/// Names the platform this client was built for.
fn current_platform() -> Platform {
    match std::env::consts::OS {
        "macos" => Platform::MacOS,
        "ios" => Platform::IOS,
        "linux" => Platform::Linux,
        "windows" => Platform::Windows,
        "freebsd" => Platform::FreeBSD,
        "emscripten" | "wasm32" => Platform::Web,
        _ => Platform::Other,
    }
}

/// Sends HELLO and waits for WELCOME on a control connection.
pub(crate) async fn handshake(
    config: &ClientConfig,
    reconnect_token: Option<String>,
    stream: &mut Stream,
    context: &ConnectionContext,
) -> Result<Welcome, ConnectionError> {
    let hello = hello(reconnect_token, config);
    let json = serde_json::to_string(&hello).map_err(|err| {
        ConnectionError::new(
            context.clone(),
            ConnectionErrorKind::Control(ControlError::InvalidHandshake(err.to_string())),
        )
    })?;

    if json.len() >= HELLO_MAX_BYTES {
        return Err(ConnectionError::new(
            context.clone(),
            ConnectionErrorKind::Control(ControlError::InvalidHandshake(format!(
                "hello is {} bytes, must be under {HELLO_MAX_BYTES} bytes",
                json.len()
            ))),
        ));
    }

    let message = timeout(HANDSHAKE_TIMEOUT, async {
        stream
            .send(tungstenite::Message::Text(json.into()))
            .await
            .map_err(|source| {
                ConnectionError::new(
                    context.clone(),
                    ConnectionErrorKind::Control(ControlError::Transport(websocket_error(
                        context, source,
                    ))),
                )
            })?;

        match stream.next().await {
            Some(Ok(message)) => Ok::<_, ConnectionError>(message),
            Some(Err(source)) => Err(ConnectionError::new(
                context.clone(),
                ConnectionErrorKind::Control(ControlError::Transport(websocket_error(
                    context, source,
                ))),
            )),
            None => Err(ConnectionError::new(
                context.clone(),
                ConnectionErrorKind::Control(ControlError::Closed),
            )),
        }
    })
    .await
    .map_err(|_| {
        ConnectionError::new(
            context.clone(),
            ConnectionErrorKind::Control(ControlError::Timeout {
                seconds: HANDSHAKE_TIMEOUT.as_secs(),
            }),
        )
    })??;

    let welcome = match message {
        tungstenite::Message::Text(text) => {
            match serde_json::from_slice::<protocol::ControlMessage>(text.as_bytes()) {
                Ok(protocol::ControlMessage::Welcome(welcome)) => welcome,
                Ok(_) => {
                    return Err(ConnectionError::new(
                        context.clone(),
                        ConnectionErrorKind::Control(ControlError::UnexpectedMessage(
                            "non-welcome message during the control handshake".into(),
                        )),
                    ));
                }
                Err(err) => {
                    return Err(ConnectionError::new(
                        context.clone(),
                        ConnectionErrorKind::Control(ControlError::InvalidHandshake(format!(
                            "invalid welcome: {err}"
                        ))),
                    ));
                }
            }
        }
        other => {
            return Err(ConnectionError::new(
                context.clone(),
                ConnectionErrorKind::Control(ControlError::UnexpectedMessage(format!(
                    "{} message during the control handshake",
                    message_name(&other)
                ))),
            ));
        }
    };

    if welcome.protocol != CONTROL_SUBPROTOCOL {
        return Err(ConnectionError::new(
            context.clone(),
            ConnectionErrorKind::Control(ControlError::InvalidHandshake(format!(
                "expected protocol {CONTROL_SUBPROTOCOL}, got {}",
                welcome.protocol
            ))),
        ));
    }

    let negotiated = welcome.api_revision.unwrap_or(0);
    if negotiated < config.api_min_revision || negotiated > config.api_revision {
        return Err(ConnectionError::new(
            context.clone(),
            ConnectionErrorKind::Control(ControlError::Revision {
                negotiated,
                min: config.api_min_revision,
                max: config.api_revision,
            }),
        ));
    }

    Ok(welcome)
}

fn message_name(message: &tungstenite::Message) -> &'static str {
    match message {
        tungstenite::Message::Text(_) => "text",
        tungstenite::Message::Binary(_) => "binary",
        tungstenite::Message::Ping(_) => "ping",
        tungstenite::Message::Pong(_) => "pong",
        tungstenite::Message::Close(_) => "close",
        tungstenite::Message::Frame(_) => "frame",
    }
}
