use bytecheck::CheckBytes;
use num_traits::FromPrimitive;
use rkyv::{
    ser::{serializers::AllocSerializer, Serializer},
    Archive, Deserialize, Infallible, Serialize,
};

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, FromPrimitive)]
#[archive_attr(derive(CheckBytes, Debug))]
pub enum BlockType {
    IBlock,
    DBlock,
    KBlock,
}

impl BlockType {
    pub fn from_u32(v: u32) -> anyhow::Result<Self> {
        match FromPrimitive::from_u32(v) {
            Some(t) => Ok(t),
            None => Err(anyhow::Error::msg("Invalid BlockType")),
        }
    }
}
