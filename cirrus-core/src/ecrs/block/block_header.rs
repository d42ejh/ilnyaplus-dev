pub use crate::ecrs::block::block_type::BlockType;
use bytecheck::CheckBytes;
use rkyv::{
    ser::{serializers::AllocSerializer, Serializer},
    Archive, Deserialize, Infallible, Serialize,
};

pub const SERIALIZED_BLOCK_HEADER_BUFFER_SIZE: usize = 4;

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct BlockHeader {
    block_type: u32,
}

impl BlockHeader {
    pub fn new(block_type: BlockType) -> Self {
        BlockHeader {
            block_type: block_type as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_header_size() {
        let h = BlockHeader::new(BlockType::DBlock);
        let mut serializer = AllocSerializer::<256>::default();
        serializer.serialize_value(&h).unwrap();
        let buffer = serializer.into_serializer().into_inner().to_vec();
        assert_eq!(buffer.len(), SERIALIZED_BLOCK_HEADER_BUFFER_SIZE);
    }
}
