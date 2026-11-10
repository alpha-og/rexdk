use serde::Serialize;

/// A client-to-server control request.
///
/// Requests are JSON text on `/api/control` and are the only message a client
/// sends after its [`crate::protocol::control::Hello`]. The server answers
/// each one with a [`crate::protocol::control::Response`] carrying the same
/// `id`.
///
/// `id` must be non-empty and must differ from the `id` of every request still
/// waiting for a response; reusing one makes the server close the connection.
/// `payload` holds the method's arguments and is omitted for methods that take
/// none. The server can run several requests at once, so responses may arrive
/// in a different order than the requests.
#[derive(Debug, Serialize)]
pub struct Request<T> {
    r#type: &'static str,
    id: String,
    method: String,
    payload: Option<T>,
}

impl<T: Serialize> Request<T> {
    /// Builds a `request` message for a method and its optional arguments.
    pub fn new(id: impl Into<String>, method: impl Into<String>, payload: Option<T>) -> Self {
        Self {
            r#type: "request",
            id: id.into(),
            method: method.into(),
            payload,
        }
    }
}
