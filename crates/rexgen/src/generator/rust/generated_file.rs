use std::path::PathBuf;

#[derive(Debug)]
pub struct GeneratedFile {
    pub path: PathBuf,
    pub contents: String,
}
