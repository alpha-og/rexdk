use rexdk::RexClient;
use rexdk::generated::{ServerStatusParams, server_status};

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
