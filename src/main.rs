#[tokio::main]
async fn main() -> anyhow::Result<()> {
    yarimashow::inbound::http::serve().await
}
