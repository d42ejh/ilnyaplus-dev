use crate::ecrs::CHK;
use cocoon_core::DHTManager;
use rocksdb::BlockBasedIndexType;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{event, Level};
use uuid::Uuid;

pub struct DownloadTask {
    pub uuid: Uuid,
    pub root_i_block_chk: CHK,
    pub working_directory: PathBuf,
}

impl DownloadTask {
    pub fn new(working_directory_root: &Path, root_i_block_chk: &CHK) -> Self {
        Self {
            uuid: Uuid::new_v4(),
            root_i_block_chk: root_i_block_chk.to_owned(),
            working_directory: working_directory_root.to_path_buf(),
        }
    }

    pub async fn start_download(&self, dht_manager: &Arc<DHTManager>) -> anyhow::Result<()> {
        use crate::ecrs::BlockType;
        event!(Level::DEBUG, "start download");

        //for now use dumb while true loop
        let mut queue = VecDeque::new();
        queue.push_back(self.root_i_block_chk.clone());

        //check if all blocks are available, if not, request through dht
        while !queue.is_empty() {
            let chk = queue.pop_front().unwrap();
            //check if available on kvdb
            todo!();
            match dht_manager.get_value_local(&chk.query)? {
                Some(data) => match BlockType::from_u32(chk.block_type)? {
                    BlockType::IBlock => {}
                    BlockType::DBlock => {}
                    BlockType::KBlock => {}
                },
                None => {
                    //request
                    dht_manager.do_find_value(&chk.query).await;
                }
            }
        }
        Ok(())
    }
    //   pub fn suspend_download()
}
