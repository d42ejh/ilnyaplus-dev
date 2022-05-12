use cocoon_core::DHTManager;
use cocoon_core::{KVDatabaseConfig, SqliteConfig};
use openssl::rand::rand_bytes;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{event, Level};
//https://www.reddit.com/r/rust/comments/f4zldz/i_audited_3_different_implementation_of_async/

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

    /// Store random data at random key.
    /// Returns stored (key,data)
    pub fn force_store(&self, key: &[u8], data: &[u8]) -> anyhow::Result<()> {
        event!(
            Level::INFO,
            "[{}] Force store: key = {}",
            self.name,
            hex::encode(key)
        );
        self.dht_manager.store_on_local(&key, &data)?;
        Ok(())
    }
}

pub struct VirtualNetworkManager {
    pub virtual_peers: Vec<Arc<VirtualPeer>>,
    last_stored_key: Arc<RwLock<Vec<u8>>>,
}

impl VirtualNetworkManager {
    pub async fn new(peers: u16) -> anyhow::Result<Self> {
        let mut vpeers = Vec::new();
        for i in 0..peers {
            let vp = Arc::new(VirtualPeer::new(&format!("vp {}", i)).await?);
            vpeers.push(vp.clone());
            vp.dht_manager.start_receive().await;
        }
        Ok(Self {
            virtual_peers: vpeers,
            last_stored_key: Arc::new(RwLock::new(vec![0; 64])),
        })
    }

    pub async fn connect_all_each_other(&self) -> anyhow::Result<()> {
        for i in 0..self.virtual_peers.len() - 1 {
            for j in i + 1..self.virtual_peers.len() {
                let vp1 = &self.virtual_peers[i];
                let vp2 = &self.virtual_peers[j];

                event!(Level::DEBUG, "ping from {} to {}", vp1.name, vp2.name);

                vp1.dht_manager
                    .do_ping(&vp2.dht_manager.local_endpoint()?)
                    .await;
            }
        }

        Ok(())
    }

    pub async fn random(&self) -> anyhow::Result<()> {
        assert!(self.virtual_peers.len() >= 2);

        use rand::prelude::*;
        //choose a random peer
        let mut rng = thread_rng();
        let choosed_index = rng.gen_range(0..self.virtual_peers.len());
        let choosed_vp = &self.virtual_peers[choosed_index];

        let others: Vec<usize> = (0..self.virtual_peers.len())
            .into_iter()
            .filter(|&i| i != choosed_index)
            .collect();

        let ri = rng.gen_range(0..others.len());
        assert_ne!(others[ri], choosed_index);
        let other_vp = &self.virtual_peers[others[ri]];

        //do something with the choosed peers
        let r = rng.gen_range(0..=3);
        match r {
            0 => {
                //ping
                event!(
                    Level::INFO,
                    "Ping request from {} to {}",
                    choosed_vp.name,
                    other_vp.name
                );
                choosed_vp
                    .dht_manager
                    .do_ping(&other_vp.dht_manager.local_endpoint()?)
                    .await;
            }
            1 => {
                //store
                event!(
                    Level::INFO,
                    "Store value request from {} to {}",
                    choosed_vp.name,
                    other_vp.name
                );
                let mut rk = vec![0; 64];
                let mut rd = vec![0; 64];
                rand_bytes(&mut rk)?;
                rand_bytes(&mut rd)?;
                choosed_vp.dht_manager.do_store(&rk, &rd).await;
                {
                    let mut w = self.last_stored_key.write().await;
                    *w = rk;
                }
            }
            2 => {
                //find value
                event!(
                    Level::INFO,
                    "Find value request from {} to {}",
                    choosed_vp.name,
                    other_vp.name
                );
                choosed_vp
                    .dht_manager
                    .do_find_value(&*self.last_stored_key.read().await)
                    .await;
            }
            3 => {
                //find node
                event!(
                    Level::INFO,
                    "Find node request from {} to {}",
                    choosed_vp.name,
                    other_vp.name
                );
                choosed_vp
                    .dht_manager
                    .do_find_node(&*self.last_stored_key.read().await)
                    .await;
            }
            _ => {
                unreachable!();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn force_store_test() -> anyhow::Result<()> {
        let vnm = VirtualNetworkManager::new(1).await?;
        let peer = &vnm.virtual_peers[0];
        let mut rk = vec![0; 64];
        let mut rd = vec![0; 64];
        rand_bytes(&mut rk)?;
        rand_bytes(&mut rd)?;
        peer.force_store(&rk, &rd)?;
        assert!(peer.dht_manager.is_available_on_local(&rk)?);
        Ok(())
    }
}
