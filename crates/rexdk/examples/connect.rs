use rexdk::RexClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _client = RexClient::connect(None::<&str>).await?;
    println!("connected to the Rex server");
    Ok(())
}
