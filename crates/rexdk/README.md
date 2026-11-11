# rexdk

A typed Rust client library for the Rex server.

[Rex website](https://www.superlogical.com/rex) · [Rex documentation](https://www.superlogical.com/rex/docs) · [Repository](https://github.com/alpha-og/rexdk)

`rexdk` is a library for programs that connect to a Rex server. It handles the
protocol and connection lifecycle so that a Rex client can be written as
application code. It opens the control connection, performs the
`hello`/`welcome` handshake, and exposes server RPCs as typed functions
generated from the server's API description. Callers deal with results,
events, and errors, not JSON messages or socket state. It can also open data
connections that stream a block's output.

The crate provides:

- `Client` and `ClientConfig` for connections, client identity, transport,
  keepalive, and event buffering.
- `Transport` for Unix socket, TCP, Tailscale, and Windows named pipe endpoints.
- `EventStream`, `events`, `DataStream`, and `DataStreamHandle` for server
  events and block output.
- `DisconnectReason`, `Error`, `ConnectionError`, and `TransportError` for
  failure reporting.
- `bindings` and `metadata` for the generated API surface and its provenance.

## Installation

```toml
[dependencies]
rexdk = { git = "https://github.com/alpha-og/rexdk.git" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Connecting

```rust
use rexdk::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::new(None);
    client.connect().await?;

    println!("connected");
    Ok(())
}
```

`Client::new` takes an optional `ClientConfig`. With `None`, the client uses
the default transport and keepalive schedule. `connect` opens the WebSocket and
completes the handshake.

`ClientConfig` fields:

| Field | Purpose |
| --- | --- |
| `kind` | Client kind sent in the handshake: `app`, `cli`, `agent`, or `other`. Defaults to `cli`. |
| `name`, `version`, `label` | Identification strings sent in the handshake. |
| `transport` | Endpoint to connect to. |
| `api_revision`, `api_min_revision` | API revision range the client supports. Both default to the revision the bindings target. |
| `keepalive` | Client ping schedule. `None` disables client pings. |
| `event_buffer_capacity` | Events buffered for a consumer that is not reading. Defaults to 64. |

### Transports

The default transport is the platform endpoint. On Unix it is the platform
data directory followed by `rex/server.sock`: `~/Library/Application Support`
on macOS, and `$XDG_DATA_HOME` or `~/.local/share` on Linux. On Windows it is
the named pipe `\\.\pipe\rex`.

`REX_SERVER` overrides the default. `Transport::from_endpoint` parses these
forms:

| Endpoint | Transport |
| --- | --- |
| `unix://<path>` | Unix socket. The path may be percent-encoded. |
| `npipe://<path>`, `\\.\pipe\<name>`, `//./pipe/<name>` | Windows named pipe. |
| `ws://host:port`, `tcp://host:port` | TCP. |
| `wss://host` | Tailscale, always port 443. |
| Any other non-empty value | A bare socket path: Unix socket on Unix, named pipe on Windows. |

An empty or malformed value is rejected by `Transport::from_endpoint`. The
default transport panics when `REX_SERVER` is set to an invalid endpoint, so
prefer an explicit `Transport` when the value comes from user input.

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

### API revision negotiation

The handshake negotiates an API revision. `ClientConfig::api_revision` and
`ClientConfig::api_min_revision` bound the revisions the client accepts and
default to the revision the generated bindings target. If the revision selected
by the server falls outside that range, `connect` fails with
`ConnectionErrorKind::Control(ControlError::Revision { .. })`. After a
successful handshake, `Client::api_revision` returns the negotiated value.

## Calling methods

A generated function performs one call and returns the typed result with the
sequence number of the last event the server had queued when it started the
request:

```rust
use rexdk::Client;
use rexdk::bindings::server::{StatusParams, status};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::new(None);
    client.connect().await?;

    let (status, seq) = status(&client, StatusParams {}).await?;
    println!(
        "Rex server {} is running (pid {})",
        status.instance_id, status.pid
    );
    println!("sessions: {}, events through seq {seq}", status.session_count);
    Ok(())
}
```

Generated modules follow the API namespaces: `session.create` is
`bindings::session::create`, and `server.status` is `bindings::server::status`.
Parameter and result types keep the method name, as in `session::CreateParams`
and `session::CreateOutput`. A method whose leaf name is a Rust keyword is
exposed as a raw identifier, such as `session::r#move`.

`help.all` is the only method with no input schema, so `help::all` takes only
the client. Methods that accept an empty object still receive an empty params
struct, such as `server::StatusParams {}`.

Generated functions take `&Client`, so one connection can serve multiple calls
and tasks. `Client::invoke` performs a call without a generated binding. After
connecting:

```rust
use rexdk::bindings::server::{StatusOutput, WhoamiOutput};

let (status, _): (StatusOutput, u64) = client.invoke("server.status", None::<()>).await?;
let (whoami, _): (WhoamiOutput, u64) = client.invoke("server.whoami", None::<()>).await?;

println!("pid: {}", status.pid);
println!("account: {:?}", whoami.account);
```

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

`take_event_stream` can be called once per `Client`. The stream survives
reconnects and ends with `None` only when the `Client` is dropped, so use
`Client::wait_disconnected` to observe a connection ending.

The reader never waits for a slow consumer. Events are buffered up to
`ClientConfig::event_buffer_capacity`, and a value of `0` is treated as `1`. If
the backlog fills, the connection ends with `DisconnectReason::EventBufferFull`
instead of dropping an event. `EventStream::lagged_event_seq` and
`Client::lagged_event_seq` return the first event that did not fit; reconnect
and read the state again before resuming.

`EventStream` implements `futures_util::Stream`, so `StreamExt::next` works as
well as `recv`.

## Ordering results and events

Events and responses share one ordered connection, but their ordering is
independent: an event can arrive before or after the response that already
reflects it. Each response carries a `seq`, the sequence number of the last
event the server had queued when it started the request, or `0` if none. That
number is the barrier between the result and concurrent events.

Every generated function returns `(Output, u64)`, and so does `Client::invoke`.
To merge a result with events:

1. Buffer events while the request is in flight instead of applying them.
2. Apply the result.
3. Discard buffered events with `seq <= barrier`; the result already includes
   them.
4. Apply the remaining events in order.

A sketch of the merge, inside the task that owns the event stream:

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
idempotent. For sessions, compare revisions and ignore changes at or below the
current one. If you never consume events, or always re-read state after a
request, `seq` can be ignored.

`seq` restarts at `1` on every control connection. After a reconnect, treat the
new sequence as a separate generation and do not compare it with values from
the old connection.

## Data connections

A data connection streams one block's output, such as a terminal. The control
connection must be attached to the block's session first. `Client::open_data`
calls `data.connect` and opens the WebSocket in one step:

```rust
use rexdk::bindings::data::ConnectParams;
use rexdk::Client;

async fn stream_terminal(
    client: &mut Client,
    session_id: String,
    block_id: String,
) -> Result<(), rexdk::Error> {
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

    // Close while the stream is still alive.
    handle.disconnect().await?;
    Ok(())
}
```

`open_data` requires `subprotocol`, because the WebSocket handshake and the
ticket must use the same value. Without it, the call fails with a connection
error whose kind is `DataError::MissingSubprotocol`.

If a ticket is already available, use `Client::connect_data(ticket,
subprotocol)`. `rexdk::decode_ticket` converts the base64url string from
`data.connect` into the ticket bytes.

The stream resumes automatically after a transport drop, using the reconnect
token and the number of bytes the worker has accepted. Accessors report its
state:

| Method | Value |
| --- | --- |
| `consumed_bytes` | Bytes delivered to the consumer, maintained across resumes. |
| `received_bytes` | Bytes the worker accepted for delivery, including buffered bytes. This is the resume offset. |
| `channel_nonce` | Channel nonce from the latest welcome, used to mark Rex terminal control sequences. |
| `resume_timeout` | How long the server retains the stream for a resume. |

Dropping the stream asks the worker to close with status 1000.
`DataStream::close().await` waits for that close to finish. Like
`EventStream`, `DataStream` implements `futures_util::Stream`.
`Client::disconnect` closes data connections before the control connection, and
`Client::disconnect_data` closes only the data connections.

Data-connection failures are `Error::Connection` with a
`ConnectionErrorKind::Data` kind. The context names the endpoint and phase
(handshake, resume, or stream). `DataError` covers rejection, a closed stream,
a reconnect requirement, an elapsed resume window, a missing subprotocol, and
protocol or transport failures. Transport failures use `TransportError` and
name the transport, endpoint, and underlying I/O or WebSocket cause.

## Staying connected

The server pings the client every 120 seconds and closes the connection with
status 1001 after three unanswered pings. WebSocket libraries answer pings
automatically; `rexdk` also sends its own pings on the same schedule so that it
notices a server that has gone away, such as after a laptop sleeps or a network
path drops without a close frame.

The defaults are `interval: 120s`, `timeout: 60s`, and `max_missed: 3`. Override
them with `ClientConfig::keepalive`, or set it to `None` to disable client
pings:

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

When the configured number of pings goes unanswered, the client closes the
connection with `DisconnectReason::KeepAliveTimeout`.

## Disconnecting

`Client::disconnect` closes the connection with a WebSocket close frame. A
close frame ends the client on the server immediately, so the server
invalidates the reconnect token. `rexdk` keeps the token, but the next
connection reports `Client::is_resumed() == false` and starts a new client.

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

`Client::wait_disconnected` resolves if the reconnect fails, so a reconnect
loop cannot hang silently. `examples/reconnect.rs` contains a complete loop.

Events that happened while disconnected are lost, and `seq` starts again at `1`
on the new connection. Re-read state with `session.view` or `session.bootstrap`
before applying new events. Requests that were in flight fail with
`Error::Connection` (see [Error handling](#error-handling)); the server may
still have run them, so check state instead of sending them again.

## Error handling

All fallible operations return `Error`. It distinguishes:

- `Error::Api` for errors returned by the server, with its `code`, HTTP-style
  `status`, and `message`.
- `Error::Connection` for connection failures, with a `ConnectionError` that
  carries the endpoint, phase, and a `ConnectionErrorKind::Control` or
  `ConnectionErrorKind::Data`.
- `Error::MessageTooLarge` when a request exceeds the welcome's
  `limits.max_message_bytes`. The request is rejected before anything is sent.
- `Error::Serialization`, `Error::Configuration`, `Error::NotConnected`, and
  other variants for local failures and misuse.

```rust
use rexdk::Error;

async fn connect_and_query() -> Result<(), Error> {
    let mut client = rexdk::Client::new(None);
    client.connect().await?;
    let (_status, _): (rexdk::bindings::server::StatusOutput, u64) =
        client.invoke("server.status", None::<()>).await?;
    Ok(())
}
```

Requests that were pending when a connection ended fail with
`Error::Connection`, whose `ControlError::Disconnected` carries the
`DisconnectReason`.

## Examples

With a Rex server running:

```sh
cargo run -p rexdk --example connect
cargo run -p rexdk --example server_status
cargo run -p rexdk --example events
cargo run -p rexdk --example reconnect
```

## Generating the bindings

`src/bindings/` is produced by `rexgen` from `api/rex-api.json` and should not
be edited manually. The bindings target the API revision recorded in the lock
file.

```sh
cargo run -p rexgen -- update-schema
cargo run -p rexgen -- generate
cargo run -p rexgen -- generate --check
```

## Limitations

- Control reconnection is manual; call `Client::reconnect` from your own loop.
  Requests are never retried automatically.
- Data streams resume automatically after a transport drop, within the server's
  resume window.
- Some optional fields in the API description are generated as
  `Option<Option<T>>`.
