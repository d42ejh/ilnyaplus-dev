use openssl::hash::{hash, MessageDigest};
use std::cmp::max;
use std::fmt;
use std::mem::size_of;
use std::net::SocketAddr;
use std::time::{Duration, SystemTime};
use tracing::{event, span, Level};
pub struct NodeInfo {
    endpoint_string: String,
}

pub struct Node {
    pub id: Vec<u8>,
    pub endpoint: SocketAddr,
    last_ping: SystemTime,
}

impl Node {
    pub fn new(sock_addr: &SocketAddr) -> Self {
        let node_id = endpoint_to_node_id(sock_addr);
        event!(
            Level::DEBUG,
            "SockAddr {}, hash {}",
            sock_addr,
            hex::encode(&node_id)
        );
        Node {
            id: node_id,
            endpoint: sock_addr.to_owned(),
            last_ping: SystemTime::now(),
        }
    }

    pub fn update_alive(&mut self) {
        self.last_ping = SystemTime::now();
    }

    pub fn is_alive(&self) -> bool {
        let one_min = Duration::from_secs(60); //TODO: for now 1 min, examine and change
        if self.last_ping.elapsed().unwrap() > one_min {
            //dead
            return false;
        }
        true
    }

    pub fn info(&self) -> NodeInfo {
        NodeInfo {
            endpoint_string: self.endpoint.to_string(),
        }
    }
}

impl fmt::Display for Node {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "endpoint: {} id: {} status: {}",
            self.endpoint,
            &hex::encode(&self.id),
            if self.is_alive() { "alive" } else { "dead" }
        )
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[must_use]
pub fn endpoint_to_node_id(endpoint: &SocketAddr) -> Vec<u8> {
    let s = endpoint.to_string();
    let sock_bytes = s.as_bytes();
    let hash = hash(MessageDigest::sha3_512(), &sock_bytes).expect("Failed to hash socket bytes");
    hash.to_vec()
}

//return true if lhs < rhs
pub fn node_id_cmp(lhs: &[u8], rhs: &[u8]) -> bool {
    assert_eq!(lhs.len(), rhs.len());
    for i in 0..lhs.len() {
        let mut l = lhs[i];
        let mut r = rhs[i];
        if cfg!(target_endian = "little") {
            l = l.to_le();
            r = r.to_le();
        } else {
            l = l.to_be();
            r = r.to_be();
        }
        if l < r {
            return true;
        }
        if l > r {
            return false;
        }
    }
    return false;
}

pub fn node_id_distance(lhs: &[u8], rhs: &[u8]) -> Vec<u8> {
    assert!(lhs.len() > 0);
    assert!(rhs.len() > 0);
    assert_eq!(lhs.len(), rhs.len());

    let mut ret = vec![0; lhs.len()];
    debug_assert_eq!(lhs.len(), ret.len());
    for i in 0..lhs.len() {
        ret[i] = lhs[i] ^ rhs[i];
    }
    ret
}

pub fn calculate_bucket_index(lhs: &[u8], rhs: &[u8]) -> usize {
    assert!(lhs.len() > 0);
    assert!(rhs.len() > 0);
    assert_eq!(lhs.len(), rhs.len());

    let distance = node_id_distance(lhs, rhs);
    let leading_zeros = u8_slice_clz(&distance);
    //calculate 2^n <= x < 2^(n+1)
    let node_id_bits = lhs.len() * 8;
    assert!(node_id_bits >= leading_zeros);
    let exp = max(node_id_bits - leading_zeros, 0);
    node_id_bits - exp
}

pub fn u8_slice_clz(v: &[u8]) -> usize {
    for i in 0..v.len() {
        if v[i] == 0 {
            //all zero
            continue;
        }
        let tmp;
        if cfg!(target_endian = "little") {
            tmp = v[i].to_le();
        } else {
            tmp = v[i].to_be();
        }
        return i * 8 + tmp.leading_zeros() as usize;
    }
    return v.len() * 8;
}

#[cfg(test)]
mod tests {
    use super::{calculate_bucket_index, u8_slice_clz};
    use openssl::rand::rand_bytes;

    #[test]
    fn u8_slice_clz_test() {
        assert_eq!(u8_slice_clz(&0_i32.to_le_bytes()), 32);
        assert_eq!(u8_slice_clz(&0_u8.to_le_bytes()), 8);
        assert_eq!(u8_slice_clz(&1_i32.to_le_bytes()), 7);
        assert_eq!(u8_slice_clz(&1_u8.to_le_bytes()), 7);

        assert_eq!(u8_slice_clz(&42_i32.to_le_bytes()), 2);
        assert_eq!(u8_slice_clz(&42_i32.to_be_bytes()), 26);

        assert_eq!(u8_slice_clz(&42_u8.to_le_bytes()), 2);
    }

    #[test]
    fn calculate_bucket_index_test() {
        let mut rb = vec![0; 64];
        rand_bytes(&mut rb).unwrap();

        assert_eq!(calculate_bucket_index(&rb, &rb), 512);
        //todo cover more (but how?)
    }
}
