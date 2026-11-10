use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

/// A server response to a [`crate::protocol::control::Request`].
///
/// A response carries the `id` of the request it answers and either `payload`
/// with the result or `error` with the failure. `seq` is the sequence number of
/// the last event the server queued before starting the request; apply events
/// with a higher `seq` on top of the result.
#[derive(Debug, Deserialize)]
pub struct Response {
    /// The `id` of the request this response answers.
    pub id: String,
    /// Last event sequence queued before the request started.
    #[allow(dead_code)] // retained for the event/response sequence barrier
    pub seq: i64,
    /// The method's result, absent when the request failed.
    pub payload: Option<Value>,
    /// Why the request failed, absent when it succeeded.
    pub error: Option<ResponseError>,
}

/// A failed request result.
///
/// Branch on `code`, not `message`: the wording is for people and can change
/// between releases. `status` has the same meaning as an HTTP status code and
/// covers codes a client does not know.
#[derive(Debug, Deserialize, Error)]
#[error("API error: {code}: {message}")]
pub struct ResponseError {
    /// Stable failure code, such as `bad_request` or `not_found`.
    pub code: String,
    /// HTTP-style status code for the failure.
    pub status: u16,
    /// Human-readable explanation.
    pub message: String,
}
