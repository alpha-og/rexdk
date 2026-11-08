# rexdk

A typed Rust client for the Rex control API.

[Rex website](https://www.superlogical.com/rex) · [Rex documentation](https://www.superlogical.com/rex/docs) · [Repository](https://github.com/alpha-og/rexdk-rs)

`rexdk` opens a control connection to a running Rex server, performs the
`hello`/`welcome` handshake, and exposes the server's methods as generated
functions with typed parameters and results.

The crate provides:

- `Client` — connection management and request/response handling.
- `ClientConfig` — client identity and socket configuration.
- `Error` — the error type returned by the client and generated bindings.
- `bindings` — one module per API namespace, with one function per API method
  and the request and response types that method requires.
- `metadata` — compile-time provenance of the generated bindings (API revision,
  Rex version, commit, schema hash).

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
use rexdk::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _client = Client::connect(None).await?;

    // Equivalent, with an explicit path:
    // let mut config = rexdk::ClientConfig::default();
    // config.socket_path = Some("/tmp/rex.sock".into());
    // let _client = Client::connect(Some(config)).await?;

    Ok(())
}
```

`connect` takes an optional `ClientConfig`. When it is `None`, the default
socket path is used.

## Calling methods

A generated function performs one call and returns the typed result:

```rust
use rexdk::Client;
use rexdk::bindings::server::{StatusParams, status};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::connect(None).await?;
    let status = status(&client, StatusParams {}).await?;

    println!(
        "Rex server {} is running (pid {})",
        status.instance_id, status.pid
    );
    println!("API revision: {:?}", status.api_revision);
    println!("Sessions: {}", status.session_count);
    Ok(())
}
```

Generated modules follow the API namespaces: `session.create` becomes
`bindings::session::create`, and `server.status` becomes
`bindings::server::status`. Parameter and result types keep the method's name,
such as `session::CreateParams` and `session::CreateOutput`. A method whose leaf
name is a Rust keyword is exposed as a raw identifier, for example
`session::r#move`.

`help.all` is the only method with no input schema, so `help::all` takes only the
client. Methods that accept an empty object still receive an empty params
struct, such as `server::StatusParams {}`.

### Client ownership

Generated functions borrow the client (`client: &Client`), so a single
connection handles any number of calls:

```rust
let status = rexdk::bindings::server::status(&client, StatusParams {}).await?;
let sessions = rexdk::bindings::session::list(&client, ListParams {}).await?;
```

`Client::invoke` is available for calls without a generated binding.

```rust
use rexdk::Client;
use rexdk::bindings::server::{StatusOutput, WhoamiOutput};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::connect(None).await?;

    let status: StatusOutput = client.invoke("server.status", None::<()>).await?;
    let whoami: WhoamiOutput = client.invoke("server.whoami", None::<()>).await?;

    println!("pid: {}", status.pid);
    println!("sessions: {}", status.session_count);
    println!("account: {:?}", whoami.account);
    Ok(())
}
```

Generated functions borrow the client, so a single connection handles any
number of calls.

## Receiving events

The server sends events on the control connection. Take the event stream once
and consume it alongside requests:

```rust
use rexdk::{
    Client,
    bindings::session::{self, BootstrapParams},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::new(None);
    client.connect().await?;

    session::bootstrap(
        &client,
        BootstrapParams {
            attach_session_ids: None,
            data_tickets: None,
            event_scope: "all".into(),
            preferred_session_id: None,
        },
    )
    .await?;

    let mut events = client.take_event_stream()?;
    while let Some(event) = events.recv().await {
        println!("{event:?}");
    }

    Ok(())
}
```

`take_event_stream` can be called once per connection. The stream ends with
`None` when the connection closes. Events must be consumed promptly; a slow
consumer applies backpressure to the reader.

## Error handling

All fallible operations return `Error`:

```rust
use rexdk::{Client, Error};

async fn connect_and_query() -> Result<(), Error> {
    let client = Client::connect(None).await?;
    let _status: rexdk::bindings::server::StatusOutput =
        client.invoke("server.status", None::<()>).await?;
    Ok(())
}
```

`Error` distinguishes connection failures, API errors returned by the
server, serialization errors, and configuration errors. API errors carry the
server's `code`, HTTP-style `status`, and `message`.

## Examples

With a Rex server running:

```sh
cargo run -p rexdk --example connect
cargo run -p rexdk --example server_status
```

## Scope and limitations

- Only the control connection is implemented. Server events are exposed through
  `Client::take_event_stream`. Data connections (`/api/data`), which stream
  block output, are not implemented.
- Only Unix domain sockets are supported. The default path is macOS-specific,
  and Windows named pipes are not implemented.
- Generated functions consume the client, as described above.
- Some optional fields in the API description are generated as
  `Option<Option<T>>`.

## Regenerating the bindings

`src/bindings/` is produced by `rexgen` from `api/rex-api.json`. It should
not be edited manually. Each API namespace is a module, so bindings are called
as `bindings::server::status(...)`.

```sh
cargo run -p rexgen -- update-schema
cargo run -p rexgen -- generate
cargo run -p rexgen -- generate --check
```
