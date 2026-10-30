use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiMethod {
    pub name: String,
    pub description: String,
    pub invoke: Invoke,
    pub input_schema: Option<Value>,
    pub output_schema: Option<Value>,
    pub request_example: Option<String>,
    pub response_example: Option<String>,
    pub validation_notes: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Invoke {
    pub method: String,
}
