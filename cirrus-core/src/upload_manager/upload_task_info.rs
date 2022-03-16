use bytecheck::CheckBytes;
use rkyv::{
    ser::{serializers::AllocSerializer, Serializer},
    Archive, Deserialize, Infallible, Serialize,
};
pub const TASK_SAVE_FILE_NAME: &str = "taskinfo";

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct UploadTaskInfo {
    pub id: String,
    pub is_encode_done: bool,
    pub is_upload_done: bool,
    pub file_path_string: String,
    pub file_size: u64,
    pub working_directory_string: String,
    pub root_i_block_chk: Option<Vec<u8>>,
}

impl UploadTaskInfo {
    pub fn from_bytes(buffer: &[u8]) -> Self {
        let archived = rkyv::check_archived_root::<UploadTaskInfo>(buffer).unwrap();
        let info: UploadTaskInfo = archived
            .deserialize(&mut Infallible)
            .expect("Failed to deserialize");
        info
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut serializer = AllocSerializer::<256>::default();
        serializer.serialize_value(self).unwrap();
        serializer.into_serializer().into_inner().to_vec()
    }
}
