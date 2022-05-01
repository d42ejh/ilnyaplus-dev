use crate::ecrs::CHK;
use std::path::{Path, PathBuf};
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
}
