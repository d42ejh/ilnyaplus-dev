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

    println!("How many peers do you want?");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    input.pop();
    let peer_count = u16::from_str(&input)?;
    assert!(peer_count >= 2);
    let vnm = VirtualNetworkManager::new(peer_count).await?;
    println!("Initialized.");

    let mut r_key = vec![0; 64];
    let mut r_data = vec![0; 64];

    loop {
        println!("Waiting for command...");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        input.pop();

        match input.as_str() {
            "ping" => {
                println!("");
            }
            "store" => {
                println!("Store");
                let vp = &vnm.virtual_peers[0];
                rand_bytes(&mut r_key).unwrap();
                rand_bytes(&mut r_data).unwrap();
                vp.dht_manager.do_store(&r_key, &r_data).await;
            }
            "fstore" => {
                println!("Force store");
                rand_bytes(&mut r_key).unwrap();
                rand_bytes(&mut r_data).unwrap();
                for i in 1..vnm.virtual_peers.len() {
                    let vp = &vnm.virtual_peers[i];
                    vp.force_store(&r_key, &r_data)?;
                }
            }
            "find" => {
                println!(
                    "Try to find a value with the recent stored key: {}",
                    hex::encode(&r_key)
                );
                let vp = &vnm.virtual_peers[0];
                vp.dht_manager.do_find_value(&r_key).await;
            }
            "connectall" => {
                println!("Connect all nodes each other");
                vnm.connect_all_each_other().await?;
            }
            _ => {
                println!("{} is not a valid command. len() = {}", input, input.len());
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
