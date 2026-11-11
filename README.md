# rexdk

A Rust SDK for the Rex server, with typed bindings generated from the server's
API description.

[Rex website](https://www.superlogical.com/rex) · [Rex documentation](https://www.superlogical.com/rex/docs) · [Repository](https://github.com/alpha-og/rexdk)

Rex runs terminal sessions on a server and keeps them running when no client
is attached. Programs control the server over a JSON WebSocket protocol and
receive block output over separate data connections.

`rexdk` is the client library in this SDK. Programs that connect to a Rex
server use it to handle protocol messages and the connection lifecycle. It
performs the handshake, matches requests to responses, delivers events, keeps
the control connection alive, and resumes data connections. Server methods are
exposed as generated, typed functions, so client code calls them and handles
results instead of constructing or parsing messages.

The workspace contains two crates:

| Crate | Role |
| --- | --- |
| [`rexdk`](crates/rexdk) | Client library for control and data connections, plus typed bindings for every server method. |
| [`rexgen`](crates/rexgen) | Generator that converts a Rex API description into the bindings used by `rexdk`. Also provides the `rexgen` CLI. |

## Installation

```toml
[dependencies]
rexdk = { git = "https://github.com/alpha-og/rexdk.git" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Rust 1.85 or newer is required. The workspace uses the 2024 edition.

## Quick start

```rust
use rexdk::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::new(None);
    client.connect().await?;
    println!("connected to the Rex server");
    Ok(())
}
```

`Client::new` takes an optional `ClientConfig`. With `None`, the client uses
the default socket path and keepalive schedule. Set `REX_SERVER` or provide an
explicit transport when the server uses a different endpoint:

```rust
use rexdk::{Client, ClientConfig, Transport};

let config = ClientConfig {
    transport: Transport::UnixSocket {
        path: "/tmp/rex.sock".into(),
    },
    ..Default::default()
};
let mut client = Client::new(Some(config));
client.connect().await?;
```

## How it works

Internally, `rexdk` separates the wire protocol from the connection runtime and
the generated API surface:

- `protocol` defines the control and data message types. It performs no I/O.
- `runtime` owns transports, connection lifecycle, request matching,
  keepalive, events, data streams, and reconnection.
- `bindings` mirrors the API namespaces. Each module defines one function per
  method, with the request and response types that method uses.

A control connection starts with a `hello` that identifies the client and the
API revisions it supports. The server answers with `welcome`, which selects the
revision for the connection and reports limits and the reconnect token. From
then on, the client sends requests and the server sends responses and events
over the same ordered connection. `Client::invoke` matches each response to its
request, and the generated functions call it with typed values.

Events are delivered through a stream taken once from the client. Each response
carries the event sequence number the server had reached when it started the
request, so a caller can merge a result with events without applying changes
twice. See [Ordering results and events](crates/rexdk/README.md#ordering-results-and-events).

Data connections use their own WebSocket and stream a block's output.
They perform a separate handshake and resume automatically after a transport
drop, within the window the server retains the stream.

A control connection is not resumed automatically. `Client::reconnect` reuses
the last reconnect token, which the server accepts for a limited time after a
connection drops without a close frame. Requests are never retried.

## Examples

| Example | Command | Description |
| --- | --- | --- |
| Connect | `cargo run -p rexdk --example connect` | Opens a control connection and completes the handshake. |
| Server status | `cargo run -p rexdk --example server_status` | Connects and calls the generated `server.status` binding. |
| Events | `cargo run -p rexdk --example events` | Connects, bootstraps the connection, and prints server events until it disconnects. |
| Reconnect | `cargo run -p rexdk --example reconnect` | Reconnects with the last token and re-bootstraps when the server starts a new client. |
| Generate bindings | `cargo run -p rexgen --example generate` | Generates Rust source from the checked-in API description. |

The crate READMEs cover usage in detail:
[`rexdk`](crates/rexdk/README.md) and [`rexgen`](crates/rexgen/README.md).

## Generating bindings

`crates/rexdk/src/bindings/` is produced from `api/rex-api.json` and verified
against `api/rex-api.lock.json`. Each API namespace becomes a Rust module, so
`session.create` is exposed as `bindings::session::create` and
`server.status` as `bindings::server::status`. Provenance is available at
compile time through `rexdk::metadata`.

```sh
# Fetch the schema and provenance from a running server.
cargo run -p rexgen -- update-schema

# Regenerate the bindings.
cargo run -p rexgen -- generate

# Fail if the checked-in bindings are stale.
cargo run -p rexgen -- generate --check
```

`rexgen generate` verifies the schema's SHA-256 against the lock file before
generating anything. The lock file records the API revision, Rex version,
server commit, and whether the server was built from a dirty tree.

## Development

```sh
cargo check --workspace --all-targets
cargo test --workspace

# Generated bindings must match the checked-in schema.
cargo run -p rexgen -- generate --check
```

## Project layout

```text
.
├── api/
│   ├── rex-api.json        # API description fetched from a Rex server
│   └── rex-api.lock.json   # Provenance for the API description
├── crates/
│   ├── rexdk/              # Runtime client and generated bindings
│   │   ├── examples/
│   │   └── src/
│   │       ├── bindings/   # Generated API modules
│   │       ├── protocol/   # Control and data wire messages
│   │       ├── runtime/    # Transport, connections, client, events
│   │       └── lib.rs
│   └── rexgen/             # Spec parser, Rust code generator, and CLI
└── Cargo.toml              # Workspace manifest
```
