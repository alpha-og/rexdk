use std::{fs::File, io::Read, path::Path};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiDescription {
    pub http: HttpEndpoints,
    pub methods: Vec<ApiMethod>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpEndpoints {
    pub help_path: String,
    pub control_path: String,
    pub data_path: String,
    pub llms_txt_path: String,
    pub llms_html_path: String,
}

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

const API_DIRECTORY: &str = "api";
const API_SPEC_FILENAME: &str = "rex-api";

fn main() -> Result<()> {
    let api_spec_filepath_raw = format!("./{API_DIRECTORY}/{API_SPEC_FILENAME}.json");
    let api_spec_filepath = Path::new(&api_spec_filepath_raw);
    dbg!(api_spec_filepath);

    let mut file = File::open(&api_spec_filepath)?;

    let mut raw_api_spec = String::new();
    file.read_to_string(&mut raw_api_spec)?;

    let api_spec: ApiDescription = serde_json::from_str(&raw_api_spec)?;
    dbg!(api_spec);
    Ok(())
}
