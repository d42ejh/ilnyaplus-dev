use cocoon_virtual::VirtualNetworkManager;
use openssl::rand::rand_bytes;
use std::str::FromStr;
use tokio::sync::mpsc;
use tracing::{event, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Initializing logger...");
    tracing_subscriber::fmt()
        .with_thread_names(true)
        .with_max_level(Level::DEBUG)
        .init();

    event!(Level::INFO, "Network simulation.");
    let vnm = VirtualNetworkManager::new(10).await?; //for now 10

    Ok(())
}
