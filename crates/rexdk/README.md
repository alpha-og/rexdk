# rexdk

A typed Rust client for the Rex API.

[Rex website](https://www.superlogical.com/rex) · [Rex documentation](https://www.superlogical.com/rex/docs) · [Repository](https://github.com/alpha-og/rexdk-rs)

`rexdk` opens a control connection to a running Rex server, performs the
`hello`/`welcome` handshake, and exposes the server's methods as generated
functions with typed parameters and results. It can also open data connections
that stream a block's output.

The crate provides:

- `Client` — connection management, request/response handling, events, data
  connections, and reconnection.
- `ClientConfig` — client identity, transport, keepalive, and event backlog configuration.
- `Transport` — Unix socket, TCP, Tailscale, or Windows named pipe endpoint.
- `DataStream` / `DataStreamHandle` — block output and its connection handle.
- `DisconnectReason` — why a connection ended.
- `Error` — the error type returned by the client and generated bindings.
- `ConnectionError` / `TransportError` — structured connection and transport
  failures.
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
`~/Library/Application Support/rex/server.sock` on macOS. Set `REX_SERVER` or
pass an explicit transport on any other platform or when the server uses a
non-default location.

```rust
use rexdk::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::new(None);
    client.connect().await?;

    // Equivalent, with an explicit transport:
    // let config = rexdk::ClientConfig {
    //     transport: rexdk::Transport::UnixSocket {
    //         path: "/tmp/rex.sock".into(),
    //     },
    //     ..Default::default()
    // };
    // let mut client = Client::new(Some(config));
    // client.connect().await?;

    Ok(())
}
```

`Client::new` takes an optional `ClientConfig`. When it is `None`, the default
socket path and keepalive schedule are used. `connect` opens the WebSocket and
performs the `hello`/`welcome` handshake.

The handshake negotiates an API revision. `ClientConfig::api_min_revision` and
`ClientConfig::api_revision` bound the revisions the client accepts and default
to the revision the generated bindings target. If the server's welcome names a
revision outside that range, `connect` fails with
`ConnectionErrorKind::Control(ControlError::Revision { .. })`. After a
successful handshake, `Client::api_revision` returns the negotiated value.

## Calling methods

A generated function performs one call and returns the typed result together
with the response's event sequence number:

```rust
use rexdk::Client;
use rexdk::bindings::server::{StatusParams, status};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::new(None);
    client.connect().await?;
    let (status, _seq) = status(&client, StatusParams {}).await?;

    println!(
        "Rex server {} is running (pid {})",
        status.instance_id, status.pid
    );
    println!("API revision: {:?}", status.api_revision);
    println!("Sessions: {}", status.session_count);
    Ok(())
}
```

Every generated function returns `(Output, u64)`. The `u64` is the `seq` of the
last event the server queued before it started the request. Use it to decide
which events a result already includes; see “Combining results with events”
below.

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
let (status, _) = rexdk::bindings::server::status(&client, StatusParams {}).await?;
let (sessions, _) = rexdk::bindings::session::list(&client, ListParams {}).await?;
```

`Client::invoke` is available for calls without a generated binding.

```rust
use rexdk::Client;
use rexdk::bindings::server::{StatusOutput, WhoamiOutput};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::new(None);
    client.connect().await?;

    let (status, _): (StatusOutput, u64) = client.invoke("server.status", None::<()>).await?;
    let (whoami, _): (WhoamiOutput, u64) = client.invoke("server.whoami", None::<()>).await?;

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
    loop {
        tokio::select! {
            event = events.recv() => {
                let Some(event) = event else { break };
                println!("{event:?}");
            }
            reason = client.wait_disconnected() => {
                println!("disconnected: {reason}");
                break;
            }
        }
    }

    Ok(())
}
```

`take_event_stream` can be called once per `Client`. The stream is stable
across reconnects and ends with `None` only when the `Client` is dropped, so
use `Client::wait_disconnected` to observe a connection ending.

The reader never waits for a slow consumer. Events are buffered up to
`ClientConfig::event_buffer_capacity` (64 by default). If the backlog fills,
the connection ends with `DisconnectReason::EventBufferFull` instead of
dropping an event. `EventStream::lagged_event_seq` and
`Client::lagged_event_seq` return the first event that did not fit; reconnect
and read the state again before resuming.

`EventStream` also implements `futures_util::Stream`, so `StreamExt::next` and
`tokio::select!` work directly.

## Combining results with events

Events and responses share one ordered connection, but their ordering is
independent: an event can arrive before or after the response that already
reflects it. Each response carries a `seq`, the sequence number of the last
event the server had queued when it *started* the request (`0` if none). That
number is the barrier.

Every generated function returns `(Output, u64)`, and so does `Client::invoke`:

```rust
let (status, response_seq) = status(&client, StatusParams {}).await?;
```

To merge a result with concurrent events:

1. Buffer events while the request is in flight instead of applying them.
2. Apply the result.
3. Discard buffered events with `seq <= response_seq`; the result already
   includes them.
4. Apply the remaining events in order.

```rust
let mut pending = Vec::new();
let (view, barrier) = loop {
    tokio::select! {
        result = session::view(&client, view_params.clone()) => break result?,
        event = events.recv() => pending.push(event.unwrap()),
    }
};

apply(&mut state, view);
for event in pending {
    if event.seq > barrier {
        apply(&mut state, event);
    }
}
```

Applying a result is not exclusive: the request runs concurrently, so an event
with `seq > barrier` may also already be reflected in the result. Make `apply`
idempotent — for sessions, compare revisions and ignore changes at or below the
current one. If you never consume events, or always re-read state after a
request, you can ignore `seq`.

`seq` restarts at `1` on every control connection. After a reconnect, treat the
new sequence as a separate generation rather than comparing it with values from
the old connection.

## Data connections

A data connection streams one block's output, such as a terminal. The control
connection must be attached to the block's session first. `Client::open_data`
calls `data.connect` and opens the WebSocket in one step:

```rust
use rexdk::bindings::data::ConnectParams;

let (mut stream, handle) = client
    .open_data(ConnectParams {
        session_id,
        block_id,
        data_ref_id: "terminal".into(),
        subprotocol: Some("rex.data.terminal.v1".into()),
    })
    .await?;

while let Some(chunk) = stream.recv().await {
    let bytes = chunk?;
    // Chunks are arbitrary parts of one continuous byte stream.
    println!("{}", bytes.len());
}

// Or, to close while the stream is still alive:
handle.disconnect().await?;
```

`open_data` requires `subprotocol` to be set, because the WebSocket and the
ticket must use the same one. If you already have a ticket, use
`Client::connect_data(ticket, subprotocol)`; `rexdk::decode_ticket` converts
the base64url string from `data.connect` into the 16 ticket bytes.

The stream resumes automatically after a transport drop, using the reconnect
token and the number of bytes the worker has accepted. `DataStream::channel_nonce`
returns the WELCOME's channel nonce, which marks Rex's own terminal control
sequences. Dropping the stream asks the worker to close with status 1000;
`DataStream::close().await` waits for it. `Client::disconnect` closes data
connections before the control connection, and `Client::disconnect_data` closes
only the data connections.

Data-connection failures are `Error::Connection` with a
`ConnectionErrorKind::Data` kind. The context names the endpoint and phase
(handshake, resume, or stream), while `DataError` distinguishes rejection,
a closed stream, a reconnect requirement, and an elapsed resume window.
Transport failures use `TransportError` and name the transport, endpoint, and
underlying I/O or WebSocket cause.

## Staying connected

The server pings the client every 120 seconds and closes the connection with
status 1001 after three unanswered pings. WebSocket libraries answer pings
automatically; `rexdk` also sends its own pings on the same schedule so that it
notices a server that has gone away, such as after a laptop sleeps or a network
path drops without a close frame.

The defaults are `interval: 120s`, `timeout: 60s`, and `max_missed: 3`, matching
the server. Override them with `ClientConfig::keepalive`, or set it to `None` to
disable client pings:

```rust
use std::time::Duration;
use rexdk::{ClientConfig, KeepAlive};

let config = ClientConfig {
    keepalive: Some(KeepAlive {
        interval: Duration::from_secs(120),
        timeout: Duration::from_secs(60),
        max_missed: 3,
    }),
    ..Default::default()
};
```

When the client misses too many pongs it closes the connection with
`DisconnectReason::KeepAliveTimeout`.

## Disconnecting

`Client::disconnect` closes the connection with a WebSocket close frame. A close
frame ends the client on the server at once, so the server invalidates the
reconnect token. `rexdk` keeps the token, but a later connection reports
`resumed: false` and starts a new client.

```rust
client.disconnect().await?;
```

Dropping the `Client` also closes the socket, but cannot send a close frame.
`Client::is_connected`, `Client::disconnect_reason`, and
`Client::wait_disconnected` report the connection state.

## Reconnecting

Rex keeps a client for 60 seconds after its connection drops without a close
frame. `Client::reconnect` drops the old socket without a close frame and opens
a new one with the token from the last welcome:

```rust
client.reconnect().await?;

if client.is_resumed() {
    // The same client and its session attachments were resumed.
} else {
    // The server started a new client; set the connection up again.
    rexdk::bindings::session::bootstrap(
        &client,
        rexdk::bindings::session::BootstrapParams {
            attach_session_ids: None,
            data_tickets: None,
            event_scope: "all".into(),
            preferred_session_id: None,
        },
    )
    .await?;
}
```

`Client::wait_disconnected` resolves if the reconnect fails, so a reconnect loop
cannot hang silently.

Events that happened while disconnected are lost, and `seq` starts again at 1
on the new connection. Re-read state with `session.view` or `session.bootstrap`
before applying new events. Requests that were in flight fail with
`Error::Connection`; the server may still have run them, so check state instead
of blindly sending them again. See `examples/reconnect.rs` for a full loop.

## Error handling

All fallible operations return `Error`:

```rust
use rexdk::{Client, Error};

async fn connect_and_query() -> Result<(), Error> {
    let mut client = Client::new(None);
    client.connect().await?;
    let (_status, _): (rexdk::bindings::server::StatusOutput, u64) =
        client.invoke("server.status", None::<()>).await?;
    Ok(())
}
```

`Error` distinguishes connection failures, API errors returned by the server,
serialization and configuration errors, and misuse such as calling
`Client::invoke` while disconnected. API errors carry the server's `code`,
HTTP-style `status`, and `message`. Requests that were pending when a connection
ended fail with `Error::Connection`, whose `ConnectionError` carries a
`ConnectionErrorKind::Control` with the `DisconnectReason`, plus the connection
context. A request larger than the welcome's `limits.max_message_bytes` fails
with `Error::MessageTooLarge` before anything is sent.

## Examples

With a Rex server running:

```sh
cargo run -p rexdk --example connect
cargo run -p rexdk --example server_status
cargo run -p rexdk --example events
cargo run -p rexdk --example reconnect
```

## Scope and limitations

- Control connections and data connections (`/api/data`) are implemented.
  Server events are exposed through `Client::take_event_stream` and block output
  through `Client::open_data` or `Client::connect_data`.
- Unix domain sockets, TCP, Tailscale, and Windows named pipes are supported
  through `Transport`. The default path is macOS-specific.
- Control reconnection is manual; call `Client::reconnect` from your own loop.
  Requests are never retried automatically. Data streams resume automatically
  after a transport drop, within the server's resume window.
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
