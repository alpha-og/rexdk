# rexdk

A typed Rust client for the Rex control API.

`rexdk` opens a control connection to a running Rex server, performs the
`hello`/`welcome` handshake, and exposes the server's methods as generated
functions with typed parameters and results.

The crate provides:

- `RexClient` — connection management and request/response handling.
- `RexError` — the error type returned by the client and generated bindings.
- `generated` — one function per API method, with the request and response
  types that method requires.

## Installation

```toml
[dependencies]
rexdk = { git = "https://github.com/alpha-og/rexdk-rs.git" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Connecting

`rexdk` connects over a Unix domain socket. Its default path is
`~/Library/Application Support/rex/server.sock`, which exists on macOS. Pass an
explicit path on any other platform or when the server uses a non-default
location.

```rust
use rexdk::RexClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _client = RexClient::connect(None::<&str>).await?;

    // Equivalent, with an explicit path:
    // let _client = RexClient::connect(Some("/tmp/rex.sock")).await?;

    Ok(())
}
```

`connect` is generic over `AsRef<Path>`. When passing `None`, the path type must
be named, for example `None::<&str>` or `None::<std::path::PathBuf>`.

## Calling methods

A generated function performs one call and returns the typed result:

```rust
use rexdk::generated::{ServerStatusParams, server_status};
use rexdk::RexClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RexClient::connect(None::<&str>).await?;
    let status = server_status(client, ServerStatusParams {}).await?;

    println!(
        "Rex server {} is running (pid {})",
        status.instance_id, status.pid
    );
    println!("API revision: {:?}", status.api_revision);
    println!("Sessions: {}", status.session_count);
    Ok(())
}
```

Generated names follow the API method names: `session.create` becomes
`session_create`, `server.status` becomes `server_status`, and so on. Parameter
and result types follow the same convention, such as `SessionCreateParams` and
`SessionCreateOutput`.

`help.all` is the only method with no input schema, so `help_all` takes only the
client. Methods that accept an empty object still receive an empty params
struct, such as `ServerStatusParams {}`.

### Client ownership

Generated functions take `client: RexClient` by value, and `RexClient` is not
`Clone`. A generated call therefore consumes the connection:

```rust
let status = server_status(client, ServerStatusParams {}).await?;
// `client` has been moved.
```

`RexClient::invoke` borrows the client instead, which allows several calls on a
single connection:

```rust
use rexdk::RexClient;
use rexdk::generated::{ServerStatusOutput, ServerWhoamiOutput};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RexClient::connect(None::<&str>).await?;

    let status: ServerStatusOutput = client.invoke("server.status", None::<()>).await?;
    let whoami: ServerWhoamiOutput = client.invoke("server.whoami", None::<()>).await?;

    println!("pid: {}", status.pid);
    println!("sessions: {}", status.session_count);
    println!("account: {:?}", whoami.account);
    Ok(())
}
```

Each generated call requires its own connection.

## Error handling

All fallible operations return `RexError`:

```rust
use rexdk::{RexClient, RexError};

async fn connect_and_query() -> Result<(), RexError> {
    let client = RexClient::connect(None::<&str>).await?;
    let _status: rexdk::generated::ServerStatusOutput =
        client.invoke("server.status", None::<()>).await?;
    Ok(())
}
```

`RexError` distinguishes connection failures, API errors returned by the
server, serialization errors, and configuration errors. API errors carry the
server's `code`, HTTP-style `status`, and `message`.

## Examples

With a Rex server running:

```sh
cargo run -p rexdk --example connect
cargo run -p rexdk --example server_status
```

## Scope and limitations

- Only the control connection is implemented. Server events are read from the
  socket but are not exposed by the public API. Data connections (`/api/data`),
  which stream block output, are not implemented.
- Only Unix domain sockets are supported. The default path is macOS-specific,
  and Windows named pipes are not implemented.
- Generated functions consume the client, as described above.
- Some optional fields in the API description are generated as
  `Option<Option<T>>`.

## Regenerating the bindings

`src/generated.rs` is produced by `rexgen` from `api/rex-api.json`. It should
not be edited manually.

```sh
cargo run -p rexgen -- update-schema
cargo run -p rexgen -- generate
cargo run -p rexgen -- generate --check
```
