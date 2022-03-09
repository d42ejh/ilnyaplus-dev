use bytecheck::CheckBytes;
use rkyv::{
    ser::{serializers::AllocSerializer, Serializer},
    Archive, Deserialize, Infallible, Serialize,
};
pub const SERIALIZED_CHK_BUFFER_SIZE: usize = 140;
use crate::ecrs::block::BlockType;

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, Clone)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct CHK {
    pub key: Vec<u8>,
    pub iv: Vec<u8>,
    pub query: Vec<u8>,
    pub block_type: u32,
    pub bf_index: u32,
}

impl CHK {
    pub fn new(key: &[u8], iv: &[u8], query: &[u8], block_type: BlockType, bf_index: u32) -> Self {
        assert_eq!(key.len(), 32);
        CHK {
            key: key.to_owned(),
            iv: iv.to_owned(),
            query: query.to_owned(),
            block_type: block_type as u32,
            bf_index: bf_index,
        }
    }

    pub fn from_bytes(buffer: &[u8]) -> Self {
        let archived = rkyv::check_archived_root::<CHK>(buffer).unwrap();
        let chk: CHK = archived
            .deserialize(&mut Infallible)
            .expect("Failed to deserialize");
        chk
    }

    pub fn serialize(&self) -> Vec<u8> {
        assert!(
            self.block_type == BlockType::DBlock as u32
                || self.block_type == BlockType::IBlock as u32
        );
        let mut serializer = AllocSerializer::<256>::default(); //TODO change 256, examine
        serializer.serialize_value(self).unwrap();
        let buffer = serializer.into_serializer().into_inner().to_vec();
        assert_eq!(buffer.len(), SERIALIZED_CHK_BUFFER_SIZE); //must
        buffer
    }
}
