use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum ClientKind {
    App,
    #[default]
    Cli,
    Agent,
    Other(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Platform {
    MacOS,
    IOS,
    Linux,
    Windows,
    FreeBSD,
    Web,
    Other(String),
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ClientConfig {
    pub kind: ClientKind,
    pub name: String,
    pub version: String,
    pub label: String,
    pub api_revision: u32,
    pub api_min_revision: u32,
    pub transport: Transport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Transport {
    UnixSocket { path: PathBuf },
    Tcp { host: String, port: u16 },
    Tailscale { hostname: String },
    NamedPipe { path: String },
}

impl Default for Transport {
    fn default() -> Self {
        #[cfg(unix)]
        {
            use crate::Error;

            let path = std::env::var_os("REX_SERVER")
                .map(PathBuf::from)
                .or_else(|| dirs::data_dir().map(|data_dir| data_dir.join("rex/server.sock")))
                .unwrap_or_else(|| panic!("{}", Error::SocketPathResolutionFailed));

            Self::UnixSocket { path }
        }

        #[cfg(windows)]
        {
            let path = std::env::var("REX_SERVER").unwrap_or_else(|_| r"\\.\pipe\rex".to_owned());

            Self::NamedPipe { path }
        }
    }
}
