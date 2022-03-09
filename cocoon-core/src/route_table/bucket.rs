use crate::route_table::node;
use node::Node;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tracing::{event, span, Level};

pub struct Bucket {
    pub nodes: VecDeque<Arc<Mutex<Node>>>,
    k: u16,
}

impl Bucket {
    pub fn new(k: u16) -> Self {
        Bucket {
            nodes: VecDeque::new(),
            k: k,
        }
    }

    pub fn add_node(&mut self, node: &Arc<Mutex<Node>>) {
        if self.is_full() {
            panic!(); //caller should verify this ^
        }
        event!(Level::DEBUG, "Add node {}", node.lock().unwrap());
        self.nodes.push_back(node.clone());
    }

    pub fn size(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_full(&self) -> bool {
        self.nodes.len() >= self.k as usize
    }

    pub fn select_nodes(&self, desired_count: usize) -> Vec<Arc<Mutex<Node>>> {
        let amount = std::cmp::min(desired_count, self.nodes.len());
        let mut nodes = Vec::new();
        for i in 0..amount {
            nodes.push(self.nodes[i].clone());
        }
        nodes
    }
}

#[cfg(test)]
mod tests {
    /*
        #[test]
        fn bucket_test() {}
    */
}
