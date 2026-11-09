use std::time::Duration;

use rexdk::{
    Client,
    bindings::session::{self, BootstrapParams},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::new(None);
    client.connect().await?;
    bootstrap(&client).await?;

    let mut events = client.take_event_stream()?;
    loop {
        let reason = tokio::select! {
            event = events.recv() => {
                let Some(event) = event else { break };
                println!("{event:?}");
                continue;
            }
            reason = client.wait_disconnected() => reason,
        };

        println!("disconnected: {reason}");

        // Rex keeps a client for 60 seconds after a connection drops without a
        // close frame. Reconnect inside that window to resume it.
        tokio::time::sleep(Duration::from_secs(1)).await;
        client.reconnect().await?;

        if client.is_resumed() {
            println!("resumed the existing Rex client");
        } else {
            println!("server started a new Rex client");
            bootstrap(&client).await?;
        }
    }

    Ok(())
}

async fn bootstrap(client: &Client) -> Result<(), rexdk::Error> {
    session::bootstrap(
        client,
        BootstrapParams {
            attach_session_ids: None,
            data_tickets: None,
            event_scope: "all".into(),
            preferred_session_id: None,
        },
    )
    .await?;
    Ok(())
}
