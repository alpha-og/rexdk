mod cli;
mod metadata;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cli::run().await
}
