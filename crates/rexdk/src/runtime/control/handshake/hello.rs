use serde::Serialize;

#[derive(Serialize)]
pub struct Hello {
    r#type: String,
    api_revision: u32,
    client: HelloClientInfo,
}

#[derive(Serialize)]
struct HelloClientInfo {
    kind: String,
    name: String,
    version: String,
}

impl Hello {
    pub fn new() -> Self {
        Self {
            r#type: "hello".into(),
            api_revision: 7,
            client: HelloClientInfo {
                kind: "cli".into(),
                name: "rexdk".into(),
                version: "0.1.0".into(),
            },
        }
    }
}
