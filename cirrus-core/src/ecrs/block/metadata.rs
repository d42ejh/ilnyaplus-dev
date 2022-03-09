use bytecheck::CheckBytes;
use rkyv::{
    ser::{serializers::AllocSerializer, Serializer},
    Archive, Deserialize, Infallible, Serialize,
};

/// TODO maybe add comment and datetime(optional)
#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, Clone)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct MetaData {
    pub file_name: String,
    pub file_size: u64,
}
