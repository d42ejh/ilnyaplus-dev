mod upload_task;
mod upload_task_info;
use crate::block_file::BlockFile;
use crate::ecrs::{encode_file_to_blocks, CHK};
use async_std::fs::OpenOptions;
use async_std::prelude::*;
use cocoon_core::DHTManager;
use quisyn::{ResumeableTask, TaskManager};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{event, Level};
use upload_task::UploadTask;
use upload_task_info::{UploadTaskInfo, TASK_SAVE_FILE_NAME};
use uuid::Uuid;

///////////////////
//pub struct TaskManager();

pub struct UploadManager {
    pub dht_manager: Arc<DHTManager>,
    pub tasks: Vec<Arc<UploadTask>>, //todo maybe delete this field
    pub task_map: HashMap<Uuid, Arc<UploadTask>>,
    working_directory: PathBuf,
}

impl UploadManager {
    pub async fn new(
        working_directory: &Path,
        dht_manager: &Arc<DHTManager>,
    ) -> anyhow::Result<Self> {
        let mut tasks = Vec::new();
        let mut task_map = HashMap::new();
        for entry in std::fs::read_dir(working_directory)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let task_save_file_path = path.join(TASK_SAVE_FILE_NAME);
            if !task_save_file_path.exists() {
                continue;
            }
            let mut file = OpenOptions::new()
                .read(true)
                .open(&task_save_file_path)
                .await
                .expect("Failed to open task save file");
            let mut buffer = Vec::new();
            file.sync_all().await?;
            file.read_to_end(&mut buffer).await?;

            //found task save directory
            let info = UploadTaskInfo::from_bytes(&buffer);
            let task = UploadTask::from_info(&info);
            let task = Arc::new(task);
            task_map.insert(task.uuid, task.clone());
            tasks.push(task);
        }
        event!(Level::DEBUG, "Found {} upload tasks", tasks.len());
        Ok(UploadManager {
            dht_manager: dht_manager.clone(),
            tasks: tasks,
            task_map: task_map,
            working_directory: working_directory.to_path_buf(),
        })
    }

    pub async fn upload(&mut self, file_path: &Path) -> anyhow::Result<()> {
        //todo maybe just panic instead
        if !file_path.exists() {
            return Err(anyhow::Error::msg(format!(
                "{} does not exist!",
                file_path.display()
            )));
        }
        if !file_path.is_absolute() {
            return Err(anyhow::Error::msg(format!(
                "{} is not an absolute path!",
                file_path.display()
            )));
        }

        if !file_path.is_file() {
            return Err(anyhow::Error::msg(format!(
                "{} is not a file!",
                file_path.display()
            )));
        }
        if !file_path.file_name().is_some() {
            return Err(anyhow::Error::msg(format!(
                "{} file name is None!",
                file_path.display()
            )));
        }
        assert!(self.working_directory.is_dir());

        //create new upload task and hold it in task_map and tasks
        let new_task = Arc::new(UploadTask::new(&self.working_directory, file_path));
        self.task_map.insert(new_task.uuid, new_task.clone());
        self.tasks.push(new_task);
        Ok(())
    }

    pub fn task_infos(&self) -> Vec<UploadTaskInfo> {
        let mut infos = Vec::new();
        for task in &self.tasks {
            infos.push(task.info());
        }
        infos
    }

    pub async fn start_task(&self, task_uuid: &Uuid) -> anyhow::Result<()> {
        let opt = self.task_map.get(task_uuid);
        if opt.is_none() {
            return Err(anyhow::Error::msg(format!(
                "Task with ID = {} is not found in upload manager!",
                task_uuid
            )));
        }
        let task = opt.unwrap();
        let task = task.clone();
        let dht_manager = self.dht_manager.clone();
        tokio::task::spawn_blocking(move || {
            tokio::spawn(async move {
                if let Err(e) = task.start_encode().await {
                    panic!("todo handle");
                    //TODO do something!
                };
                //upload
                if let Err(e) = task.upload(&dht_manager).await {
                    panic!("todo handle");
                    //TODO do something!
                }
            });
        });
        Ok(())
    }
}

//experimental, WIP
impl ResumeableTask for UploadTask {
    fn resume(&self) {
        //TODO
        
    }
    fn is_done(&self) -> bool {
        false
    }
    fn save(&self) -> Vec<u8> {
        let info = self.info();
        info.to_bytes()
    }
    fn load(savedata: &[u8]) -> Self {
        let info = UploadTaskInfo::from_bytes(savedata);
        let task = UploadTask::from_info(&info);
        task
    }
}
