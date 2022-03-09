use crate::ecrs::block::block_header::{BlockHeader, BlockType};
use crate::ecrs::block::metadata::MetaData;
use crate::ecrs::chk::CHK;
use bytecheck::CheckBytes;
use rkyv::{
    ser::{serializers::AllocSerializer, Serializer},
    Archive, Deserialize, Infallible, Serialize,
};

pub const IBLOCK_CHK_CAPACITY: u64 = 256;
pub const MAX_ENCRYPTED_IBLOCK_BUFFER_SIZE: usize = 35880;

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct IBlock {
    pub header: BlockHeader,
    pub chks: Vec<CHK>,
    pub metadata: Option<MetaData>,
}

impl IBlock {
    pub fn new(chks: &[CHK]) -> Self {
        assert!(0 < chks.len());
        assert!(chks.len() as u64 <= IBLOCK_CHK_CAPACITY);
        IBlock {
            header: BlockHeader::new(BlockType::IBlock),
            chks: chks.to_vec(),
            metadata: None,
        }
    }

    pub fn new_root(chks: &[CHK], metadata: &MetaData) -> Self {
        assert!(0 < chks.len());
        assert!(chks.len() as u64 <= IBLOCK_CHK_CAPACITY);
        IBlock {
            header: BlockHeader::new(BlockType::IBlock),
            chks: chks.to_vec(),
            metadata: Some(metadata.clone()),
        }
    }

    pub fn from_bytes(buffer: &[u8]) -> Self {
        let archived = rkyv::check_archived_root::<IBlock>(buffer).expect("Invalid data");
        let block: IBlock = archived.deserialize(&mut Infallible).unwrap();
        block
    }
}
