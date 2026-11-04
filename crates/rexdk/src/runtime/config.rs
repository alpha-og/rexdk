use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub enum ClientKind {
    App,
    #[default]
    Cli,
    Agent,
    Other(String),
}

#[derive(Debug, Serialize)]
pub enum Platform {
    MacOS,
    IOS,
    Linux,
    Windows,
    FreeBSD,
    Web,
    Other(String),
}

#[derive(Debug, Default)]
pub struct ClientConfig {
    pub kind: ClientKind,
    pub name: String,
    pub version: String,
    pub label: String,
    pub api_revision: u32,
    pub api_min_revision: u32,
    pub socket_path: Option<PathBuf>,
}
