use cocoon_virtual::VirtualNetworkManager;
use openssl::rand::rand_bytes;
use tracing::{event, Level};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn find_value_test() -> anyhow::Result<()> {
    //logger
    tracing_subscriber::fmt()
        .with_thread_names(true)
        .with_max_level(Level::DEBUG)
        .init();

    let vnm = VirtualNetworkManager::new(2).await?;
    vnm.connect_all_each_other().await?;

    let vp0 = &vnm.virtual_peers[0];
    let vp1 = &vnm.virtual_peers[1];

    //store random data on vp1
    let mut rkey = vec![0; 64];
    let mut rdata = vec![0; 64];
    rand_bytes(&mut rkey)?;
    rand_bytes(&mut rdata)?;
    vp1.force_store(&rkey, &rdata)?;

    //find value
    vp0.dht_manager.do_find_value(&rkey).await;

    Ok(())
}
