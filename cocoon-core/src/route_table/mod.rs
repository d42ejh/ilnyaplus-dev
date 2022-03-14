mod bucket;
mod node;
use bucket::Bucket;
pub use node::{calculate_bucket_index, endpoint_to_node_id, node_id_cmp, node_id_distance, Node};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Mutex;
use std::{net::SocketAddr, sync::Arc};
use tracing::{event, span, Level};

/// RouteTable
/// RouteTable for DHTManager.
pub struct RouteTable {
    k: u16,
    own_node: Node,
    buckets: Vec<Bucket>, //TODO: maybe this isn't necessarily vector
    /// Useful for checking whether a node is in the buckets or not.
    node_map: HashMap<SocketAddr, Arc<Mutex<Node>>>,
}

impl Drop for RouteTable {
    fn drop(&mut self) {
        event!(Level::DEBUG, "Dropping route table");
    }
}

impl RouteTable {
    #[must_use]
    pub fn new(own_endpoint: &SocketAddr, k: u16, buckets_capacity: usize) -> Self {
        //create buckets
        let mut buckets = Vec::with_capacity(buckets_capacity);
        for i in 0..buckets_capacity {
            buckets.push(Bucket::new(k));
        }

        RouteTable {
            k: k,
            own_node: Node::new(own_endpoint),
            buckets: buckets,
            node_map: HashMap::new(),
        }
    }

    #[must_use]
    pub fn contains(&self, endpoint: &SocketAddr) -> bool {
        self.node_map.contains_key(endpoint)
    }

    #[must_use]
    pub fn get_node_by_endpoint(&self, endpoint: &SocketAddr) -> Arc<Mutex<Node>> {
        assert!(self.contains(endpoint));
        let opt = self.node_map.get(&endpoint);
        assert!(opt.is_some());
        opt.unwrap().clone()
    }

    //
    pub fn add_node(&mut self, node_endpoint: &SocketAddr) -> anyhow::Result<bool> {
        event!(Level::DEBUG, "add node");
        let new_node = Node::new(node_endpoint);

        assert!(self.own_node != new_node);

        if self.contains(node_endpoint) {
            //already in route table
            //update status
            event!(Level::ERROR, "{} is already in route table", node_endpoint);
            let node = self.get_node_by_endpoint(node_endpoint);
            {
                //update node status
                let mut node = node.lock().unwrap();
                node.update_alive();
            }
            event!(Level::DEBUG, "Updated the status of {}", node_endpoint);
            return Ok(true);
        }

        //find bucket for node
        let bucket = self.find_bucket_mut_ref(&new_node.id);

        let new_node = Arc::new(Mutex::new(new_node));

        if bucket.is_full() {
            return Ok(false);
        }

        //add to bucket
        bucket.add_node(&new_node);
        //add to node map
        self.node_map.insert(*node_endpoint, new_node);
        Ok(true)
    }

    #[must_use]
    pub fn find_bucket(&self, id: &[u8]) -> &Bucket {
        event!(Level::DEBUG, "Find bucket");

        let index = calculate_bucket_index(&self.own_node.id, &id);

        event!(Level::DEBUG, "bucket index {}", index);

        if self.buckets.len() <= index {
            panic!("Logic error, need bigger bucket vector\ncalculated bucket index {}\ncurrent bucket vector capacity {}", index,self.buckets.len());
        }
        assert!(0 as usize <= index && index < self.buckets.len());

        &self.buckets[index]
    }

    #[must_use]
    pub fn find_bucket_mut_ref(&mut self, id: &[u8]) -> &mut Bucket {
        event!(Level::DEBUG, "Find bucket");
        let index = calculate_bucket_index(&self.own_node.id, &id);
        event!(Level::DEBUG, "bucket index {}", index);
        if self.buckets.len() <= index {
            panic!("Logic error, need bigger bucket vector\ncalculated bucket index {}\ncurrent bucket vector capacity {}", index,self.buckets.len());
        }
        assert!(0 as usize <= index && index < self.buckets.len());

        &mut self.buckets[index]
    }

    #[must_use]
    pub fn find_nodes(&self, id: &[u8], desired_count: usize) -> Vec<Arc<Mutex<Node>>> {
        debug_assert!(id.len() != 0);
        let bucket = self.find_bucket(&id);
        bucket.select_nodes(desired_count)
        //todo if nodes.len() < desired_count
        //maybe gather from other buckets
    }

    //todo write test
    #[must_use]
    pub fn is_closest_to(&self, id: &[u8]) -> bool {
        let bucket = self.find_bucket(&id);
        for node in bucket.select_nodes(bucket.size()) {
            let node = node.lock().unwrap();
            let d1 = node_id_distance(&self.own_node.id, &id);
            let d2 = node_id_distance(&node.id, &id);
            let not_closest = node_id_cmp(&d2, &d1); //true if d2 < d1
            if not_closest {
                return false;
            }
        }
        return true;
    }

    #[must_use]
    pub fn is_space_available_for(&self, endpoint: &SocketAddr) -> bool {
        let id = endpoint_to_node_id(&endpoint);
        let bucket = self.find_bucket(&id);
        !bucket.is_full()
    }

    pub fn save(&self) {
        for bucket in &self.buckets {
            for node in &bucket.nodes {
                let node = node.lock().unwrap();
                let node_info = node.info();
                //TODO
                event!(Level::WARN, "TODO code route tabhle save");
            }
        }
    }
}
