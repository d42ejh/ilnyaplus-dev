use cocoon_virtual::VirtualNetworkManager;
use tracing::{event, Level};


/// Start two virtual peers and test the ping functions.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ping_test() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_thread_names(true)
        .with_max_level(Level::DEBUG)
        .init();

    let vnm = VirtualNetworkManager::new(2).await?;
    std::thread::sleep(std::time::Duration::from_secs(2));

    let vp1 = &vnm.virtual_peers[0];
    let vp2 = &vnm.virtual_peers[1];

    assert_eq!(
        vp1.dht_manager
            .route_table
            .lock()
            .await
            .contains(&vp2.dht_manager.local_endpoint()),
        false
    );

    assert_eq!(
        vp2.dht_manager
            .route_table
            .lock()
            .await
            .contains(&vp1.dht_manager.local_endpoint()),
        false
    );

    // ping
    vnm.connect_all_each_other().await?;
    std::thread::sleep(std::time::Duration::from_secs(3));

    assert_eq!(
        vp1.dht_manager
            .route_table
            .lock()
            .await
            .contains(&vp2.dht_manager.local_endpoint()),
        true
    );

    assert_eq!(
        vp2.dht_manager
            .route_table
            .lock()
            .await
            .contains(&vp1.dht_manager.local_endpoint()),
        true
    );

    Ok(())
}
