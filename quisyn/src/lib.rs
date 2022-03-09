use std::collections::HashMap;
use std::{fmt::Result, hash::Hash};
use uuid::Uuid;
pub trait ResumeableTask {
    fn resume(&self);
    fn is_done(&self) -> bool;
    fn save(&self) -> Vec<u8>;
    fn load(savedata: &[u8]) -> Self;
}

pub struct TaskManager<T>
where
    T: ResumeableTask,
{
    tasks: HashMap<Uuid, T>,
}

impl<T: ResumeableTask> TaskManager<T> {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }
    pub fn append_task(&mut self, task: T) -> Uuid {
        let uuid = Uuid::new_v4();
        self.tasks.insert(uuid, task);
        uuid
    }
}

#[cfg(test)]
mod tests {
    use crate::{ResumeableTask, TaskManager};
    // use serde::{Deserialize, Serialize};
    use rkyv::{
        archived_root,
        ser::{serializers::AllocSerializer, Serializer},
        Archive, Deserialize, Infallible, Serialize,
    };
    use std::fs::File;
    use std::path::{Path, PathBuf};

    #[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
    struct DevJob {
        str: String,
    }

    impl DevJob {
        pub fn new() -> Self {
            Self { str: String::new() }
        }
    }

    impl ResumeableTask for DevJob {
        fn resume(&self) {
            //simulate heavy job
            std::thread::sleep(std::time::Duration::from_secs(5));
        }

        fn is_done(&self) -> bool {
            false
        }

        fn save(&self) -> Vec<u8> {
            let mut serializer = AllocSerializer::<256>::default();
            serializer.serialize_value(self).unwrap();
            serializer.into_serializer().into_inner().to_vec()
        }

        fn load(savedata: &[u8]) -> Self {
            let archived = unsafe { archived_root::<Self>(savedata) }; //todo use safe api
            let ret: Self = archived.deserialize(&mut Infallible).unwrap();
            ret
        }
    }

    #[test]
    fn dev_test() -> anyhow::Result<()> {
        //create temp file
        let cd = std::env::current_dir()?;
        let mut file_path = cd.clone();
        file_path.push("testfile");
        let file = File::create(&file_path)?;
        let job = DevJob::new();

        let mut tm = TaskManager::new();
        tm.append_task(job);
        Ok(())
    }
}
