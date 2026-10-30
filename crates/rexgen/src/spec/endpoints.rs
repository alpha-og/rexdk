use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiEndpoints {
    pub help_path: String,
    pub control_path: String,
    pub data_path: String,
    pub llms_txt_path: String,
    pub llms_html_path: String,
}
