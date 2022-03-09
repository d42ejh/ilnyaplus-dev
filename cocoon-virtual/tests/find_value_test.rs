use cocoon_virtual::VirtualNetworkManager;
use openssl::rand::rand_bytes;

#[tokio::test]
async fn find_value_test() -> anyhow::Result<()> {
    //prepare a dummy data
    let mut rkey = vec![0; 64];
    let mut rdata = vec![0; 64];
    rand_bytes(&mut rkey)?;
    rand_bytes(&mut rdata)?;

    let vnm = VirtualNetworkManager::new(2).await?;
    let vp = &vnm.virtual_peers[0];
    vp.dht_manager.do_find_value(&rkey).await;

    //todo store data first
    Ok(())
}
