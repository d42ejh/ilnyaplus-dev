use crate::block_file::BlockFile;
use crate::ecrs::{encode_file_to_blocks, CHK};
use crate::upload_manager::upload_task_info;
use async_std::fs::OpenOptions;
use async_std::prelude::*;
use cocoon_core::DHTManager;
use rkyv::ser::{serializers::AllocSerializer, Serializer};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{event, Level};
use upload_task_info::{UploadTaskInfo, TASK_SAVE_FILE_NAME};
use uuid::Uuid;

/// Represents upload task.
pub struct UploadTask {
    pub uuid: Uuid,
    pub file_path: PathBuf,
    pub file_size: u64,
    pub is_encode_done: Arc<Mutex<bool>>, //I don't want to bother with atomic
    pub is_upload_done: Arc<Mutex<bool>>,
    pub working_directory: PathBuf,
    pub root_i_block_chk: Option<CHK>,
}

impl UploadTask {
    pub fn new(working_directory_root: &Path, file_path: &Path) -> Self {
        //maybe return result or panic
        assert!(file_path.exists());
        assert!(file_path.is_file());
        assert!(working_directory_root.is_dir());
        assert!(working_directory_root.exists());

        let metadata = file_path.metadata().unwrap();
        let file_size = metadata.len();
        let task_uuid = Uuid::new_v4();

        //create task working directory with the task uuid
        let task_working_dir = working_directory_root.join(&task_uuid.to_string());
        std::fs::create_dir(&task_working_dir).expect(&format!(
            "Failed to create task working directory at {:?}",
            &task_working_dir
        ));

        event!(
            Level::DEBUG,
            "Created a new task at {:?}",
            &task_working_dir
        );
        UploadTask {
            uuid: task_uuid,
            file_path: file_path.to_owned(),
            file_size: file_size,
            is_encode_done: Arc::new(Mutex::new(false)),
            is_upload_done: Arc::new(Mutex::new(false)),
            working_directory: task_working_dir,
            root_i_block_chk: None,
        }
    }

    pub fn from_info(info: &UploadTaskInfo) -> Self {
        UploadTask {
            uuid: Uuid::parse_str(&info.id).unwrap(),
            file_path: PathBuf::from(&info.file_path_string),
            file_size: info.file_size,
            is_encode_done: Arc::new(Mutex::new(info.is_encode_done)),
            is_upload_done: Arc::new(Mutex::new(info.is_upload_done)),
            working_directory: PathBuf::from(&info.working_directory_string),
            root_i_block_chk: match &info.root_i_block_chk {
                Some(v) => Some(CHK::from_bytes(v)),
                None => None,
            },
        }
    }

    /// This function is intended to called from tokio::task::spawnblocking
    /// So it is okay to block long time in this function
    pub async fn start_encode(&self) -> anyhow::Result<()> {
        event!(Level::DEBUG, "Start encode!!!!!!!!!!!");
        let is_encode_done;
        {
            is_encode_done = *self.is_encode_done.lock().unwrap();
        }
        if !is_encode_done {
            //encode file
            let file_path = self.file_path.clone();
            let working_directory = self.working_directory.clone();
            //open as read only

            let root_i_block_chk = encode_file_to_blocks(&file_path, &working_directory).await?;

            /*
            TODO
                        //Do KBlock stuffs
                        let dummy_keywords: Vec<String> = Vec::new(); //TODO implement

                        for keyword in &dummy_keywords {
                            let k_block = KBlock::new(&root_i_block_chk, &keyword);
                        }
            */

            event!(Level::DEBUG, "Done encoding of {:?}", file_path);
            {
                *self.is_encode_done.lock().unwrap() = true;
            }
            //save task
            self.save().await;
        }
        //encode is done, start upload task
        event!(Level::DEBUG, "TODO upload");
        Ok(())
    }

    //upload encoded blocks with DHTManager
    pub async fn upload(&self, dht_manager: &Arc<DHTManager>) -> anyhow::Result<()> {
        //todo make block file names constants
        //here and in ecrs::encode_file_to_blocks

        let d_block_bf_path = self.working_directory.join("blocks.d");
        let d_block_chk_bf_path = self.working_directory.join("blocks.d.chk");

        let i_block_bf_path = self.working_directory.join("blocks.i");
        let i_block_chk_bf_path = self.working_directory.join("blocks.i.chk");

        debug_assert!(d_block_bf_path.is_file());
        debug_assert!(d_block_chk_bf_path.is_file());
        debug_assert!(i_block_bf_path.is_file());
        debug_assert!(i_block_chk_bf_path.is_file());

        //upload DBlocks and its CHKs

        //open bf files
        let mut d_block_bf = BlockFile::open(&d_block_bf_path).await?;
        let mut d_block_chk_bf = BlockFile::open(&d_block_chk_bf_path).await?;
        debug_assert_eq!(d_block_bf.n(), d_block_chk_bf.n());

        for i in 0..d_block_bf.n() as usize {
            let d_block_chk = CHK::from_bytes(&d_block_chk_bf.read_nth_block(i).await?);
            let encrypted_d_block_buffer = d_block_bf.read_nth_block(i).await?;
            dht_manager
                .do_store(&d_block_chk.key, &encrypted_d_block_buffer)
                .await; //upload DBlock
        }

        drop(d_block_bf);
        drop(d_block_chk_bf);

        //open bf files
        let mut i_block_bf = BlockFile::open(&i_block_bf_path).await?;
        let mut i_block_chk_bf = BlockFile::open(&i_block_chk_bf_path).await?;
        //IBlocks
        for i in 0..i_block_bf.n() as usize {
            let encrypted_i_block_buffer = i_block_bf.read_nth_block(i).await?;
            let i_block_chk = CHK::from_bytes(&i_block_chk_bf.read_nth_block(i).await?);
            dht_manager
                .do_store(&i_block_chk.key, &encrypted_i_block_buffer)
                .await; //upload IBlock
        }

        event!(Level::DEBUG, "Done uploading of {:?}", self.uuid);

        {
            *self.is_upload_done.lock().unwrap() = true; //done uploading
        }
        Ok(())
    }

    pub fn info(&self) -> UploadTaskInfo {
        UploadTaskInfo {
            id: self.uuid.to_string(),
            is_encode_done: *self.is_encode_done.lock().unwrap(),
            is_upload_done: *self.is_upload_done.lock().unwrap(),
            file_path_string: self.file_path.to_str().unwrap().to_owned(),
            file_size: self.file_size,
            working_directory_string: self.working_directory.to_str().unwrap().to_owned(),
            root_i_block_chk: match &self.root_i_block_chk {
                Some(chk) => Some(chk.serialize()),
                None => None,
            },
        }
    }

    /// Save task to file
    /// TODO: maybe use std::fs::File instead of async_std File
    pub async fn save(&self) {
        let info = self.info();
        let serialized_info_buffer;
        //serialize
        //do not remove this brace
        {
            let mut serializer = AllocSerializer::<256>::default(); //TODO: For now 256, examine and change
            serializer
                .serialize_value(&info)
                .expect("Failed to serialize a message");
            serialized_info_buffer = serializer.into_serializer().into_inner().to_vec();
        }
        //write to file
        let save_file_path = self.working_directory.join(TASK_SAVE_FILE_NAME);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(true)
            .create(true) //create if missing
            .open(&save_file_path)
            .await
            .unwrap();
        file.sync_all().await.unwrap();
        file.write_all(&serialized_info_buffer).await.unwrap();
    }
}
