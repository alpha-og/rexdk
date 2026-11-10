use crate::runtime::api::control::{Hello, Welcome};

/// A client-to-server control handshake message.
///
/// The server's WELCOME is handled through [`crate::runtime::api::Message`],
/// which also carries responses and events.
#[derive(Debug)]
#[allow(dead_code)]
pub enum Message {
    Hello(Hello),
    Welcome(Welcome),
}
