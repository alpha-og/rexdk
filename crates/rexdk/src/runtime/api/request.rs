use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Request<T> {
    r#type: &'static str,
    id: String,
    method: String,
    payload: Option<T>,
}

impl<T: Serialize> Request<T> {
    pub fn new(id: impl Into<String>, method: impl Into<String>, payload: Option<T>) -> Self {
        Self {
            r#type: "request",
            id: id.into(),
            method: method.into(),
            payload,
        }
    }
}
