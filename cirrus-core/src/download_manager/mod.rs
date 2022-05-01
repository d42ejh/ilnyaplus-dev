mod download_task;
use crate::ecrs;
use cocoon_core::DHTManager;
use download_task::DownloadTask;
use ecrs::CHK;
use std::path::{Path, PathBuf};
/// Manages jobs(download files, upload files)
//maybe change this to download manager
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct DownloadManager {
    pub dht_manager: Arc<DHTManager>,
    pub tasks: Vec<DownloadTask>,
    pub working_directory: PathBuf,
}

impl DownloadManager {
    pub async fn new(working_directory: &Path, dht_manager: &Arc<DHTManager>) -> Self {
        //todo read all download tasks from file to vector
        //implement task serialize
        DownloadManager {
            tasks: Vec::new(),
            working_directory: working_directory.to_path_buf(),
            dht_manager: dht_manager.clone(),
        }
    }

    // Register a donwload task
    pub async fn download(&self, top_i_block_chk: &CHK) {
        //todo
    }
}
