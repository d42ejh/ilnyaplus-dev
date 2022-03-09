use cocoon_virtual::VirtualNetworkManager;
use tracing::{event, Level};

#[tokio::test]
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
    let (rkey, rdata) = vp1.force_store()?;

    //find value
    vp0.dht_manager.do_find_value(&rkey).await;

    Ok(())
}
