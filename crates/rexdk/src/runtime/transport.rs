use std::path::PathBuf;

use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_tungstenite::{
    WebSocketStream, client_async,
    tungstenite::{client::IntoClientRequest, http::header::SEC_WEBSOCKET_PROTOCOL},
};

use crate::{
    Client, Error,
    Transport::{NamedPipe, Tailscale, Tcp, UnixSocket},
    runtime::error::ConnectionError,
};

pub trait AsyncStream: AsyncRead + AsyncWrite {}

impl<T> AsyncStream for T where T: AsyncRead + AsyncWrite {}

type BoxedStream = Box<dyn AsyncStream + Unpin + Send>;
pub type Stream = WebSocketStream<BoxedStream>;

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
                    let endpoint = endpoint.to_str().expect("REX_SERVER is not valid UTF-8");
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

fn connect_error(
    transport: &str,
    endpoint: impl std::fmt::Display,
    source: std::io::Error,
) -> Error {
    Error::Connection(ConnectionError::Connect {
        transport: format!("{transport} {endpoint}"),
        source,
    })
}

/// Opens a WebSocket to a Rex endpoint over `transport`.
///
/// This is used both for the control connection and for data connections,
/// including the reconnects a data connection performs without a `Client`.
pub(crate) async fn connect_transport(
    transport: &Transport,
    endpoint: &str,
    subprotocol: &str,
) -> Result<Stream, Error> {
    let (stream, uri): (BoxedStream, String) =
        match transport {
            UnixSocket { path } => {
                #[cfg(unix)]
                {
                    use tokio::net::UnixStream;

                    let stream =
                        Box::new(UnixStream::connect(path).await.map_err(|source| {
                            connect_error("unix socket", path.display(), source)
                        })?);
                    let uri = format!("ws://localhost{endpoint}");
                    (stream, uri)
                }

                #[cfg(not(unix))]
                {
                    use crate::runtime::error::ConnectionError::UnsupportedTransport;
                    return Err(Error::Connection(UnsupportedTransport {
                        transport: "UnixSocket".into(),
                        platform: "Windows".into(),
                    }));
                }
            }
            Tcp { host, port } => {
                let stream =
                    Box::new(TcpStream::connect((host.as_str(), *port)).await.map_err(
                        |source| connect_error("tcp", format!("{host}:{port}"), source),
                    )?);
                (stream, format!("ws://{host}:{port}{endpoint}"))
            }
            Tailscale { hostname } => {
                let stream = Box::new(
                    TcpStream::connect((hostname.as_str(), 443))
                        .await
                        .map_err(|source| connect_error("tailscale", hostname, source))?,
                );
                (stream, format!("wss://{hostname}{endpoint}"))
            }

            NamedPipe { path } => {
                #[cfg(windows)]
                {
                    let stream = Box::new(
                        ClientOptions::new()
                            .open(path)
                            .map_err(|source| connect_error("named pipe", path, source))?,
                    );
                    (stream, format!("ws://localhost{endpoint}"))
                }

                #[cfg(not(windows))]
                {
                    use crate::runtime::error::ConnectionError::UnsupportedTransport;

                    let _ = path;
                    return Err(Error::Connection(UnsupportedTransport {
                        transport: "NamedPipe".into(),
                        platform: "Non Windows".into(),
                    }));
                }
            }
        };
    let mut request = uri.into_client_request()?;
    request
        .headers_mut()
        .insert(SEC_WEBSOCKET_PROTOCOL, subprotocol.parse()?);
    let (ws, _) = client_async(request, stream).await?;
    Ok(ws)
}

impl Client {
    pub async fn create_transport(
        &self,
        endpoint: &str,
        subprotocol: &str,
    ) -> Result<Stream, Error> {
        connect_transport(&self.config.transport, endpoint, subprotocol).await
    }
}
