use rkyv::{
    ser::{serializers::AllocSerializer, Serializer},
    Archive, Deserialize, Infallible, Serialize,
};
use bytecheck::CheckBytes;

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, FromPrimitive)]
#[archive_attr(derive(CheckBytes, Debug))]
pub enum BlockType {
    IBlock,
    DBlock,
    KBlock,
}