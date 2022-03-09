use cocoon_core::DHTManager;
use cocoon_core::{KVDatabaseConfig, SqliteConfig};
use openssl::rand::rand_bytes;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{event, Level};

pub struct VirtualPeer {
    pub dht_manager: Arc<DHTManager>,
    pub name: String,
}

impl VirtualPeer {
    pub async fn new(name: &str) -> anyhow::Result<Self> {
        let dummy = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0);
        let mut db_path = std::env::current_dir()?;
        db_path.push("kvdb_".to_owned() + name);
        let dummy_config = KVDatabaseConfig { db_path };
        let sqlite_config = SqliteConfig {
            db_path: PathBuf::from(":memory:"),
        };
        Ok(Self {
            dht_manager: Arc::new(DHTManager::new(&dummy_config, &sqlite_config, &dummy).await?),
            name: name.to_string(),
        })
    }

    /// Use dht-dev feature and store random data at random key.
    /// Returns stored (key,data)
    pub fn force_store(&self) -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
        let mut rkey = vec![0; 64];
        let mut rdata = vec![0; 64];
        rand_bytes(&mut rkey).unwrap();
        rand_bytes(&mut rdata).unwrap();
        self.dht_manager.dev_store(&rkey, &rdata)?;
        Ok((rkey, rdata))
    }
}

pub struct VirtualNetworkManager {
    pub virtual_peers: Vec<Arc<VirtualPeer>>,
}

impl VirtualNetworkManager {
    pub async fn new(peers: u16) -> anyhow::Result<Self> {
        let mut vpeers = Vec::new();
        for i in 0..peers {
            let vp = Arc::new(VirtualPeer::new(&format!("vp {}", i)).await?);
            vpeers.push(vp.clone());
            tokio::spawn(async move {
                vp.dht_manager.start_receive().await;
            });
        }
        Ok(Self {
            virtual_peers: vpeers,
        })
    }

    pub async fn connect_all_each_other(&self) -> anyhow::Result<()> {
        for i in 0..self.virtual_peers.len() {
            for j in i..self.virtual_peers.len() {
                let vp1 = &self.virtual_peers[i];
                let vp2 = &self.virtual_peers[j];
                vp1.dht_manager
                    .do_ping(&vp2.dht_manager.local_endpoint())
                    .await;
                event!(Level::DEBUG, "ping from {} to {}", vp1.name, vp2.name);
            }
        }

        Ok(())
    }
    /*
    pub async fn store(&self, peer_index: usize) -> anyhow::Result<()> {
        self.virtual_peers[peer_index]
            .dht_manager
            .do_store(key, data);
        Ok(())
    }
    */
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn force_store_test() -> anyhow::Result<()> {
        let vnm = VirtualNetworkManager::new(1).await?;
        let peer = &vnm.virtual_peers[0];
        let (k, v) = peer.force_store()?;
        assert!(peer.dht_manager.is_available_on_local(&k)?);
        Ok(())
    }
}
