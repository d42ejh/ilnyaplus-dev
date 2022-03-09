use cocoon_virtual::VirtualNetworkManager;
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

    println!("How many peers do you want?");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    input.pop();
    let peer_count = u16::from_str(&input)?;
    let vnm = VirtualNetworkManager::new(peer_count).await?;
    println!("Initialized.");

    loop {
        println!("Waiting for command...");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        input.pop();

        match input.as_str() {
            "ping" => {}
            "store" => {}
            "find" => {}
            "connectall" => {
                vnm.connect_all_each_other().await?;
            }
            _ => {
                println!("{} is not a valid command. len() = {}", input, input.len());
            }
        }
    }
}
