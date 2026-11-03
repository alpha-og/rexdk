use rexdk::runtime::RexClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    RexClient::connect().await?;
    Ok(())
}
