use crate::ecrs::block::block_header::{BlockHeader, BlockType};
use bytecheck::CheckBytes;
use rkyv::{
    ser::{serializers::AllocSerializer, Serializer},
    Archive, Deserialize, Infallible, Serialize,
};

pub const DBLOCK_SIZE_IN_BYTES: u64 = 32 * 1024; //32kb
pub const MAX_ENCRYPTED_DBLOCK_BUFFER_SIZE: usize = 32780;

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct DBlock {
    pub header: BlockHeader,
    pub data: Vec<u8>,
}

impl DBlock {
    pub fn new(data: &[u8]) -> DBlock {
        DBlock {
            header: BlockHeader::new(BlockType::DBlock),
            data: data.to_vec(),
        }
    }

    pub fn from_bytes(buffer: &[u8]) -> Self {
        let archived = rkyv::check_archived_root::<DBlock>(buffer).unwrap();
        let block: DBlock = archived.deserialize(&mut Infallible).unwrap();
        block
    }
}
