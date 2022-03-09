mod download_task;
use crate::ecrs;
use download_task::DownloadTask;
use ecrs::CHK;
use std::path::{Path, PathBuf};
/// Manages jobs(download files, upload files)
//maybe change this to download manager

pub struct DownloadManager {
    pub tasks: Vec<DownloadTask>,
    pub working_directory: PathBuf,
}

impl DownloadManager {
    pub async fn new(working_directory: &Path) -> Self {
        //todo read all download tasks from file to vector
        //implement task serialize
        DownloadManager {
            tasks: Vec::new(),
            working_directory: working_directory.to_path_buf(),
        }
    }

    // Register a donwload task
    pub async fn download(&self, top_i_block_chk: &CHK) {
        //todo
    }
}
