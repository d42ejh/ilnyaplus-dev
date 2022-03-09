use cocoon_virtual::VirtualNetworkManager;

#[tokio::test]
async fn ping_test() -> anyhow::Result<()> {
    let vnm = VirtualNetworkManager::new(2).await?;

    Ok(())
}
