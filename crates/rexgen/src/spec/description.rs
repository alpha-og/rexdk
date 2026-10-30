use serde::{Deserialize, Serialize};

use crate::spec::{endpoints::ApiEndpoints, method::ApiMethod};

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiDescription {
    pub http: ApiEndpoints,
    pub methods: Vec<ApiMethod>,
}
