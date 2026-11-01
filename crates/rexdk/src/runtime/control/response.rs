use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct Response {
    pub r#type: String,
    pub id: String,
    pub seq: i64,
    pub payload: Option<Value>,
    pub error: Option<ResponseError>,
}

#[derive(Debug, Deserialize)]
pub struct ResponseError {
    pub code: String,
    pub status: u16,
}
