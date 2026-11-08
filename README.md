# rexdk-rs

A Rust SDK and binding generator for the Rex control API.

[Rex website](https://www.superlogical.com/rex) · [Rex documentation](https://www.superlogical.com/rex/docs) · [Repository](https://github.com/alpha-og/rexdk-rs)

Rex is a server that runs terminal sessions and keeps them alive independently
of any attached client. It exposes a JSON control protocol over a local
WebSocket, which clients use to inspect and manipulate server state.

The workspace contains two crates:

| Crate | Role |
| --- | --- |
| [`rexdk`](crates/rexdk) | Runtime client for the control connection, plus typed bindings for every server method. |
| [`rexgen`](crates/rexgen) | Generator that converts a Rex API description into the bindings used by `rexdk`. Also provides the `rexgen` CLI. |

`rexdk` consists of a small runtime (`Client`, `ClientConfig`, `Error`) and
`bindings/`, a module tree that mirrors the API namespaces and defines one
function per API method together with the request and response types that
method requires.

## Requirements

- Rust 1.85 or newer. The workspace uses the 2024 edition.
- A running Rex server. The client's default socket path is
  `~/Library/Application Support/rex/server.sock`, which exists on macOS.

## Installation

```toml
[dependencies]
rexdk = { git = "https://github.com/alpha-og/rexdk-rs.git" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Quick start

```rust
use rexdk::RexClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _client = RexClient::connect(None::<&str>).await?;
    println!("connected to the Rex server");
    Ok(())
}
```

`connect` takes an optional socket path. When the default path is not valid,
pass one explicitly:

```rust
let _client = RexClient::connect(Some("/tmp/rex.sock")).await?;
```

## Examples

| Example | Command | Description |
| --- | --- | --- |
| Connect | `cargo run -p rexdk --example connect` | Opens a control connection and completes the handshake. |
| Server status | `cargo run -p rexdk --example server_status` | Connects and calls the generated `server.status` binding. |
| Events | `cargo run -p rexdk --example events` | Connects, bootstraps the connection, and prints server events. |
| Generate bindings | `cargo run -p rexgen --example generate` | Generates Rust source from the checked-in API description. |

The crate-level READMEs cover usage in detail:
[`rexdk`](crates/rexdk/README.md) and [`rexgen`](crates/rexgen/README.md).

## Regenerating the bindings

`crates/rexdk/src/bindings/` is produced from `api/rex-api.json` and
verified against `api/rex-api.lock.json`. Each API namespace becomes a Rust
module (`bindings::session`, `bindings::server::apple`, …). Provenance is
available at compile time through `rexdk::metadata` (`METADATA`, `API_REVISION`,
`SCHEMA_SHA256`, …).

```sh
# Fetch the schema and provenance from a running server.
cargo run -p rexgen -- update-schema

# Regenerate the bindings.
cargo run -p rexgen -- generate

# Fail if the checked-in bindings are stale.
cargo run -p rexgen -- generate --check
```

`rexgen generate` verifies the spec's SHA-256 against the lock file before
generating anything. The lock file records the API revision, the Rex version,
the server commit, and whether the server was built from a dirty tree.

The checked-in spec is API revision 10.

## Project layout

```text
.
├── api/
│   ├── rex-api.json        # API description fetched from a Rex server
│   └── rex-api.lock.json   # API revision, Rex version, commit, and spec hash
├── crates/
│   ├── rexdk/              # Runtime client and generated bindings
│   │   ├── examples/
│   │   └── src/
│   │       ├── bindings/
│   │       ├── lib.rs
│   │       └── runtime/
│   └── rexgen/             # Spec parser, Rust code generator, and CLI
└── Cargo.toml              # Workspace manifest
```

## Scope and limitations

- Only the control connection is implemented. Server events are exposed through
  `Client::take_event_stream`. Data connections (`/api/data`), which stream
  block output, are not implemented.
- Only Unix domain sockets are supported. The default path is macOS-specific,
  and Windows named pipes are not implemented.
- Generated method functions borrow the client (`client: &Client`), so several
  calls can share one connection. `Client::invoke` is available for calls
  without a generated binding.
- Some optional fields in the API description are generated as
  `Option<Option<T>>`.

## Development

```sh
cargo check --workspace --all-targets
cargo test --workspace

# Generated bindings must match the checked-in spec.
cargo run -p rexgen -- generate --check
```
