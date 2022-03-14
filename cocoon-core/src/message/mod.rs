use crate::constant;
use bytecheck::CheckBytes;
use rkyv::{
    ser::{serializers::AllocSerializer, Serializer},
    Archive, Deserialize, Infallible, Serialize,
};
use std::net::SocketAddr;
use tracing::{event, Level};

/// Network messages.

/// Network message types.
#[derive(Debug, PartialEq, Eq, FromPrimitive)]
pub enum MessageType {
    PingRequest = 1,
    FindNodeRequest = 2,
    FindValueRequest = 3,
    StoreValueRequest = 4,
    PingResponse = 5,
    FindNodeResponse = 6,
    FindValueResponse = 7,
}

/// Network message header.
/// For now, only contains a message_type.
#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct MessageHeader {
    pub message_type: u32,
}

impl MessageHeader {
    pub fn new(message_type: MessageType) -> Self {
        MessageHeader {
            message_type: message_type as u32,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() >= constant::MESSAGE_HEADER_SIZE);
        let archived =
            rkyv::check_archived_root::<Self>(&bytes[0..constant::MESSAGE_HEADER_SIZE]).unwrap();
        let header: Self = archived.deserialize(&mut Infallible).unwrap();
        header
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut serializer = AllocSerializer::<256>::default(); //todo bench
        serializer
            .serialize_value(self)
            .expect("Failed to serialize a message");
        let av = serializer.into_serializer().into_inner();
        assert_eq!(av.len(), constant::MESSAGE_HEADER_SIZE);

        av.to_vec()
    }
}
//TODO: maybe it is possible to refactor these with traits or enum

/// Ping request message.
/// Peers which received this will reply with PingResponseMessage.
#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct PingRequestMessage {}

impl PingRequestMessage {
    pub fn new() -> Self {
        PingRequestMessage {}
    }

    pub fn from_bytes(bytes: &[u8]) -> (MessageHeader, Self) {
        let header = MessageHeader::from_bytes(bytes);
        let archived =
            rkyv::check_archived_root::<Self>(&bytes[constant::MESSAGE_HEADER_SIZE..]).unwrap();
        let msg: Self = archived.deserialize(&mut Infallible).unwrap();
        (header, msg)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let header = MessageHeader::new(MessageType::PingRequest);
        let mut bytes = header.to_bytes();

        let mut serializer = AllocSerializer::<32>::default(); //todo bench
        serializer
            .serialize_value(self)
            .expect("Failed to serialize a message");
        bytes.extend_from_slice(&serializer.into_serializer().into_inner());
        bytes
    }
}

///
#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct FindNodeRequestMessage {
    pub key: Vec<u8>,
}

impl FindNodeRequestMessage {
    pub fn new(key: &[u8]) -> Self {
        FindNodeRequestMessage { key: key.to_vec() }
    }

    pub fn from_bytes(bytes: &[u8]) -> (MessageHeader, Self) {
        let header = MessageHeader::from_bytes(bytes);
        let archived =
            rkyv::check_archived_root::<Self>(&bytes[constant::MESSAGE_HEADER_SIZE..]).unwrap();
        let msg: Self = archived.deserialize(&mut Infallible).unwrap();

        (header, msg)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let header = MessageHeader::new(MessageType::FindNodeRequest);
        let mut bytes = header.to_bytes();
        let mut serializer = AllocSerializer::<512>::default(); //todo bench
        serializer
            .serialize_value(self)
            .expect("Failed to serialize a message");
        bytes.extend_from_slice(&serializer.into_serializer().into_inner());
        bytes
    }
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct FindValueRequestMessage {
    pub key: Vec<u8>,
}

impl FindValueRequestMessage {
    pub fn new(key: &[u8]) -> Self {
        debug_assert!(key.len() != 0);
        FindValueRequestMessage {
            key: key.to_owned(),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> (MessageHeader, Self) {
        let header = MessageHeader::from_bytes(bytes);
        let archived =
            rkyv::check_archived_root::<Self>(&bytes[constant::MESSAGE_HEADER_SIZE..]).unwrap();
        let msg: Self = archived.deserialize(&mut Infallible).unwrap();
        (header, msg)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let header = MessageHeader::new(MessageType::FindValueRequest);
        let mut bytes = header.to_bytes();
        println!("find val header {} bytes", bytes.len());
        let mut serializer = AllocSerializer::<512>::default(); //todo bench
        serializer
            .serialize_value(self)
            .expect("Failed to serialize a message");
        let body_bytes = serializer.into_serializer().into_inner();

        println!("find val body {} bytes", body_bytes.len());

        bytes.extend_from_slice(&body_bytes);
        bytes
    }
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct StoreValueRequestMessage {
    pub key: Vec<u8>,
    pub data: Vec<u8>,
    pub replication_level: u32,
    //TODO expire date
}

impl StoreValueRequestMessage {
    pub fn new(key: &[u8], data: &[u8], replication_level: u32) -> Self {
        StoreValueRequestMessage {
            key: key.to_vec(),
            data: data.to_vec(),
            replication_level: replication_level,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> (MessageHeader, Self) {
        let header = MessageHeader::from_bytes(bytes);
        let archived =
            rkyv::check_archived_root::<Self>(&bytes[constant::MESSAGE_HEADER_SIZE..]).unwrap();
        let msg: Self = archived.deserialize(&mut Infallible).unwrap();
        (header, msg)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let header = MessageHeader::new(MessageType::StoreValueRequest);
        let mut bytes = header.to_bytes();
        let mut serializer = AllocSerializer::<512>::default(); //todo bench
        serializer
            .serialize_value(self)
            .expect("Failed to serialize a message");
        bytes.extend_from_slice(&serializer.into_serializer().into_inner());
        bytes
    }
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct PingResponseMessage {}

impl PingResponseMessage {
    pub fn new() -> Self {
        PingResponseMessage {}
    }

    pub fn from_bytes(bytes: &[u8]) -> (MessageHeader, Self) {
        let header = MessageHeader::from_bytes(bytes);
        let archived =
            rkyv::check_archived_root::<Self>(&bytes[constant::MESSAGE_HEADER_SIZE..]).unwrap();
        let msg: Self = archived.deserialize(&mut Infallible).unwrap();
        (header, msg)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let header = MessageHeader::new(MessageType::PingResponse);
        let mut bytes = header.to_bytes();
        let mut serializer = AllocSerializer::<512>::default(); //todo bench
        serializer
            .serialize_value(self)
            .expect("Failed to serialize a message");
        bytes.extend_from_slice(&serializer.into_serializer().into_inner());
        bytes
    }
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct FindNodeResponseMessage {
    pub nodes: Vec<SocketAddr>,
}

impl FindNodeResponseMessage {
    pub fn new(addrs: &[SocketAddr]) -> Self {
        FindNodeResponseMessage {
            nodes: addrs.to_vec(),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> (MessageHeader, Self) {
        let header = MessageHeader::from_bytes(bytes);
        let archived =
            rkyv::check_archived_root::<Self>(&bytes[constant::MESSAGE_HEADER_SIZE..]).unwrap();
        let msg: Self = archived.deserialize(&mut Infallible).unwrap();
        (header, msg)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let header = MessageHeader::new(MessageType::FindNodeResponse);
        let mut bytes = header.to_bytes();
        let mut serializer = AllocSerializer::<512>::default(); //todo bench
        serializer
            .serialize_value(self)
            .expect("Failed to serialize a message");
        bytes.extend_from_slice(&serializer.into_serializer().into_inner());
        bytes
    }
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub struct FindValueResponseMessage {
    pub key: Vec<u8>,
    pub node: Option<SocketAddr>,
    pub data: Option<Vec<u8>>,
}

impl FindValueResponseMessage {
    pub fn new(key: &[u8], node: Option<&SocketAddr>, data: Option<&[u8]>) -> Self {
        assert!(!(node.is_none() && data.is_none()));
        assert!(!(node.is_some() && data.is_some()));
        if node.is_some() && data.is_none() {
            FindValueResponseMessage {
                key: key.to_vec(),
                node: Some(*node.unwrap()),
                data: None,
            }
        } else {
            FindValueResponseMessage {
                key: key.to_vec(),
                node: None,
                data: Some(data.unwrap().to_vec()),
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> (MessageHeader, Self) {
        let header = MessageHeader::from_bytes(bytes);
        let archived =
            rkyv::check_archived_root::<Self>(&bytes[constant::MESSAGE_HEADER_SIZE..]).unwrap();
        let msg: Self = archived.deserialize(&mut Infallible).unwrap();
        (header, msg)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let header = MessageHeader::new(MessageType::FindValueResponse);
        let mut bytes = header.to_bytes();
        let mut serializer = AllocSerializer::<512>::default(); //todo bench
        serializer
            .serialize_value(self)
            .expect("Failed to serialize a message");
        bytes.extend_from_slice(&serializer.into_serializer().into_inner());
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::constant::MESSAGE_HEADER_SIZE;
    use super::{FindNodeRequestMessage, MessageHeader, MessageType, PingRequestMessage};
    use crate::message::{FindValueRequestMessage, PingResponseMessage, StoreValueRequestMessage};
    use openssl::rand::rand_bytes;

    #[test]
    pub fn header() -> anyhow::Result<()> {
        let h = MessageHeader::new(MessageType::PingRequest);
        assert_eq!(h.message_type, MessageType::PingRequest as u32);

        //serialize
        let bytes = h.to_bytes();
        assert_eq!(bytes.len(), MESSAGE_HEADER_SIZE);

        //deserialize
        let hh = MessageHeader::from_bytes(&bytes);

        assert_eq!(h, hh);
        Ok(())
    }

    #[test]
    pub fn ping_request() -> anyhow::Result<()> {
        //header
        let header = MessageHeader::new(MessageType::PingRequest);

        let req = PingRequestMessage::new();

        let bytes = req.to_bytes();
        let (h, r) = PingRequestMessage::from_bytes(&bytes);
        assert_eq!(h, header);
        assert_eq!(r, req);
        Ok(())
    }

    #[test]
    pub fn find_node_request() -> anyhow::Result<()> {
        //header
        let header = MessageHeader::new(MessageType::FindNodeRequest);

        let mut key = vec![0; 64];
        rand_bytes(&mut key)?;
        let req = FindNodeRequestMessage::new(&key);

        assert_eq!(key, req.key);

        let bytes = req.to_bytes();
        let (h, r) = FindNodeRequestMessage::from_bytes(&bytes);
        assert_eq!(h, header);
        assert_eq!(r, req);

        Ok(())
    }

    #[test]
    pub fn find_value_request() -> anyhow::Result<()> {
        //header
        let header = MessageHeader::new(MessageType::FindValueRequest);

        let mut key = vec![0; 64];
        rand_bytes(&mut key)?;

        let req = FindValueRequestMessage::new(&key);
        assert_eq!(key, req.key);

        let bytes = req.to_bytes();
        let (h, r) = FindValueRequestMessage::from_bytes(&bytes);
        assert_eq!(h, header);
        assert_eq!(r, req);

        Ok(())
    }

    #[test]
    pub fn store_value_request() -> anyhow::Result<()> {
        //header
        let header = MessageHeader::new(MessageType::StoreValueRequest);

        let mut key = vec![0; 64];
        let mut data = vec![0; 64];
        rand_bytes(&mut key)?;
        rand_bytes(&mut data)?;
        let rep_level = 99;
        let req = StoreValueRequestMessage::new(&key, &data, rep_level);
        assert_eq!(key, req.key);
        assert_eq!(data, req.data);
        assert_eq!(rep_level, req.replication_level);

        let bytes = req.to_bytes();
        let (h, r) = StoreValueRequestMessage::from_bytes(&bytes);
        assert_eq!(h, header);
        assert_eq!(r, req);

        Ok(())
    }

    #[test]
    pub fn ping_response() -> anyhow::Result<()> {
        //header
        let header = MessageHeader::new(MessageType::PingResponse);

        let req = PingResponseMessage::new();

        let bytes = req.to_bytes();
        let (h, r) = PingResponseMessage::from_bytes(&bytes);
        assert_eq!(h, header);
        assert_eq!(r, req);
        Ok(())
    }

    //todo other response message
}
