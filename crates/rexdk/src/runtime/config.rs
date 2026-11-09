use std::{path::PathBuf, time::Duration};

use percent_encoding::percent_decode_str;
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientConfig {
    pub kind: ClientKind,
    pub name: String,
    pub version: String,
    pub label: String,
    pub api_revision: u32,
    pub api_min_revision: u32,
    pub transport: Transport,
    /// Schedule for client-side pings. `None` disables them.
    pub keepalive: Option<KeepAlive>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            kind: ClientKind::default(),
            name: String::new(),
            version: String::new(),
            label: String::new(),
            api_revision: 0,
            api_min_revision: 0,
            transport: Transport::default(),
            keepalive: Some(KeepAlive::default()),
        }
    }
}

/// Keepalive settings for the control connection.
///
/// Rex pings the client every 120 seconds and closes the connection with
/// status 1001 after three unanswered pings. The client sends its own pings on
/// the same schedule so that it notices a server that has gone away, such as
/// after a laptop sleeps or a network path drops without a close frame.
///
/// WebSocket libraries answer pings automatically; this type only controls the
/// client's own schedule. A ping that is not answered within [`Self::timeout`]
/// counts as missed, and after [`Self::max_missed`] consecutive misses the
/// client closes the connection with [`crate::DisconnectReason::KeepAliveTimeout`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeepAlive {
    /// How long to wait between pings.
    pub interval: Duration,
    /// How long to wait for a pong before counting a ping as missed.
    pub timeout: Duration,
    /// Consecutive missed pings allowed before the connection is closed.
    pub max_missed: u32,
}

impl Default for KeepAlive {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(120),
            timeout: Duration::from_secs(60),
            max_missed: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Transport {
    UnixSocket { path: PathBuf },
    Tcp { host: String, port: u16 },
    Tailscale { hostname: String },
    NamedPipe { path: String },
}

impl Transport {
    /// Parses a server endpoint, such as the value of `REX_SERVER`.
    ///
    /// `unix://<path>` and `npipe://<path>` name a socket, and their paths may
    /// be percent-encoded, as in `%20` for a space. `ws://host:port` and
    /// `tcp://host:port` open a TCP connection. `wss://host` opens a TLS
    /// connection through Tailscale; only port 443 is supported. Any other
    /// value is treated as a bare socket path.
    ///
    /// Returns `None` for an empty endpoint or a malformed URL.
    pub fn from_endpoint(endpoint: &str) -> Option<Self> {
        if let Some(path) = endpoint.strip_prefix("unix://") {
            let path = percent_decode_str(path).decode_utf8().ok()?;
            return Some(Self::UnixSocket {
                path: PathBuf::from(path.as_ref()),
            });
        }

        if let Some(path) = endpoint.strip_prefix("npipe://") {
            return Some(Self::NamedPipe {
                path: path.to_owned(),
            });
        }

        if endpoint.starts_with(r"\\.\pipe\") || endpoint.starts_with("//./pipe/") {
            return Some(Self::NamedPipe {
                path: endpoint.to_owned(),
            });
        }

        if let Some(authority) = endpoint
            .strip_prefix("ws://")
            .or_else(|| endpoint.strip_prefix("tcp://"))
        {
            let (host, port) = parse_authority(authority)?;
            return Some(Self::Tcp { host, port });
        }

        if let Some(authority) = endpoint.strip_prefix("wss://") {
            let authority = authority.split('/').next()?;
            if authority.is_empty() {
                return None;
            }

            let (hostname, port) = match authority.rsplit_once(':') {
                Some((hostname, port)) => (hostname, port.parse::<u16>().ok()?),
                None => (authority, 443),
            };
            if port != 443 {
                return None;
            }

            return Some(Self::Tailscale {
                hostname: hostname.to_owned(),
            });
        }

        if endpoint.is_empty() {
            return None;
        }

        #[cfg(windows)]
        {
            Some(Self::NamedPipe {
                path: endpoint.to_owned(),
            })
        }

        #[cfg(not(windows))]
        {
            Some(Self::UnixSocket {
                path: PathBuf::from(endpoint),
            })
        }
    }
}

fn parse_authority(authority: &str) -> Option<(String, u16)> {
    let authority = authority.split('/').next()?;
    let (host, port) = authority.rsplit_once(':')?;
    if host.is_empty() {
        return None;
    }
    Some((host.to_owned(), port.parse().ok()?))
}

impl Default for Transport {
    fn default() -> Self {
        #[cfg(unix)]
        {
            use crate::Error;

            match std::env::var_os("REX_SERVER") {
                Some(endpoint) => match endpoint.to_str() {
                    Some(endpoint) => Self::from_endpoint(endpoint)
                        .unwrap_or_else(|| panic!("invalid REX_SERVER endpoint: {endpoint}")),
                    // A bare path need not be valid UTF-8.
                    None => Self::UnixSocket {
                        path: PathBuf::from(endpoint),
                    },
                },
                None => Self::UnixSocket {
                    path: dirs::data_dir()
                        .map(|data_dir| data_dir.join("rex/server.sock"))
                        .unwrap_or_else(|| panic!("{}", Error::SocketPathResolutionFailed)),
                },
            }
        }

        #[cfg(windows)]
        {
            match std::env::var_os("REX_SERVER") {
                Some(endpoint) => {
                    let endpoint = endpoint
                        .to_str()
                        .expect("REX_SERVER is not valid UTF-8");
                    Self::from_endpoint(endpoint)
                        .unwrap_or_else(|| panic!("invalid REX_SERVER endpoint: {endpoint}"))
                }
                None => Self::NamedPipe {
                    path: r"\\.\pipe\rex".to_owned(),
                },
            }
        }
    }
}
