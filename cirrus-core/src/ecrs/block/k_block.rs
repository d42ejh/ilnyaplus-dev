use crate::ecrs::{BlockHeader, BlockType, CHK};
use bytecheck::CheckBytes;
use rkyv::{
    ser::{serializers::AllocSerializer, Serializer},
    Archive, Deserialize, Infallible, Serialize,
};

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct KBlock {
    pub header: BlockHeader,
    pub keyword: String,
    pub chk: CHK, //root IBlock's CHK
}

impl KBlock {
    pub fn new(chk: &CHK, keyword: &str) -> KBlock {
        KBlock {
            header: BlockHeader::new(BlockType::KBlock),
            keyword: keyword.to_owned(),
            chk: chk.clone(),
        }
    }

    pub fn from_bytes(buffer: &[u8]) -> anyhow::Result<Self> {
        let archived = rkyv::check_archived_root::<KBlock>(buffer).unwrap();
        let block: KBlock = archived.deserialize(&mut Infallible)?;
        Ok(block)
    }
}
