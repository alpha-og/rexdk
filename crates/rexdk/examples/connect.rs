use rexdk::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _client = Client::connect(None).await?;
    println!("connected to the Rex server");
    Ok(())
}
