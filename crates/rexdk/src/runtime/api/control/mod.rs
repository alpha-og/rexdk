//! Control-connection wire messages.
//!
//! Every control message is one JSON object in one WebSocket text message on
//! `/api/control`, named by its `type`. The server sends `welcome`, `response`,
//! and `event`; the client sends one `hello` and then only `request`
//! messages. Skip a `type` you do not know, because a newer server may add one.

pub mod events;
mod hello;
mod message;
mod request;
mod response;
mod welcome;

pub use hello::Hello;
pub use message::ControlMessage;
pub use request::Request;
pub use response::{Response, ResponseError};
pub use welcome::Welcome;
