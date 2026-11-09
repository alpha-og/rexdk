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
