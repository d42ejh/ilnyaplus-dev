use crate::cocoon_config;
use crate::constant;
use crate::message;
use crate::route_table;
use crate::utility;
use anyhow::{anyhow, Result};
use cocoon_config::{KVDatabaseConfig, SqliteConfig};
use constant::MESSAGE_HEADER_SIZE;
use message::*;
use rocksdb::{Options, ReadOptions, WriteOptions, DB};
use route_table::{endpoint_to_node_id, RouteTable};
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tracing::{event, span, Level};
const DHT_DATA_COLUMN_FAMILY: &str = "dht-data-cf";

/// DHTManager
/// TODO implement route table save&load (with file)
pub struct DHTManager {
    pub route_table: Arc<Mutex<RouteTable>>,
    udp_socket: Arc<UdpSocket>,
    kvdb: Arc<DB>,
    db: std::sync::Mutex<Connection>,
    ping_list: Arc<std::sync::Mutex<HashSet<SocketAddr>>>,
}

impl DHTManager {
    pub async fn new(
        kvdb_config: &KVDatabaseConfig,
        sqlite_config: &SqliteConfig,
        ownep: &SocketAddr,
    ) -> Result<Self> {
        //kvdb options
        let mut db_options = Options::default();
        db_options.create_if_missing(true);
        db_options.create_missing_column_families(true);
        //open kvdb
        let kvdb = DB::open_cf(&db_options, &kvdb_config.db_path, [DHT_DATA_COLUMN_FAMILY]).expect(
            &format!("Failed to open the kvdb: {:?}", &kvdb_config.db_path),
        );

        // open sqlite
        let db = Connection::open(&sqlite_config.db_path)?;

        //udpsocket
        let sock =
            UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0)).await?;

        //use bound addr as own ep (for cocoon virtual)
        let cls = || {
            if cfg!(feature = "dht-dev") {
                event!(Level::WARN, "Executing codes for dht-dev");
                return sock.local_addr().unwrap();
            } else {
                return *ownep;
            }
        };
        Ok(DHTManager {
            route_table: Arc::new(Mutex::new(RouteTable::new(&cls(), 20, 77))),
            udp_socket: Arc::new(sock),
            kvdb: Arc::new(kvdb),
            db: std::sync::Mutex::new(db),
            ping_list: Arc::new(std::sync::Mutex::new(HashSet::new())),
        })
    }

    //TODO: maybe separate each match handlers to functions
    //TODO: test with malformed messages
    /// Start receiving messages from network
    pub async fn start_receive(&self) {
        let cloned_socket = self.udp_socket.clone();
        let cloned_route_table = self.route_table.clone();
        let cloned_kvdb = self.kvdb.clone();
        let cloned_ping_list = self.ping_list.clone();
        tokio::spawn(async move {
            loop {
                let mut buffer = vec![0; 50000]; //todo define max size
                event!(Level::DEBUG, "Waiting for incoming message...");
                let (received_size, sender) = cloned_socket
                    .recv_from(&mut buffer)
                    .await
                    .expect("Failed to receive"); //TODO: maybe separate receive cycle and handle cycle

                //resize buffer(truncate)
                debug_assert!(received_size <= buffer.len());
                buffer.resize(received_size, 0xff);

                //deserialize message header
                if buffer.len() < MESSAGE_HEADER_SIZE {
                    //malformed
                    //TODO: maybe block sender
                    continue;
                }

                let message_header = MessageHeader::from_bytes(&buffer);

                let msg_type: Option<MessageType> =
                    num::FromPrimitive::from_u32(message_header.message_type);

                if msg_type.is_none() {
                    todo!("Message type is 'none' TODO should handle this");
                }
                match msg_type.unwrap() {
                    MessageType::PingRequest => {
                        event!(Level::DEBUG, "Received ping request from {}", &sender);
                        //TODO: should I add the sender to route table?
                        // for now add

                        {
                            let mut rt = cloned_route_table.lock().await;
                            let is_handled = rt.add_node(&sender).unwrap();
                            if !is_handled {
                                event!(Level::DEBUG, "Space not available for the new node");
                                let bucket = rt.find_bucket(&endpoint_to_node_id(&sender));
                                for node in &bucket.nodes {
                                    let ep;
                                    {
                                        let node = node.lock().unwrap();
                                        ep = node.endpoint;
                                    }
                                    {
                                        let mut ping_list = cloned_ping_list.lock().unwrap();
                                        //insert to ping list
                                        ping_list.insert(ep);
                                    }
                                    if do_ping_impl(&cloned_socket, &ep).await.is_err() {
                                        event!(Level::ERROR, "Failed to ping");
                                    }
                                }

                                //todo
                                event!(
                                    Level::WARN,
                                    "TODO: remove dead nodes from the route table and add new node"
                                );
                                return;
                            }
                        }
                        //send ping reply(pong)
                        pong(&cloned_socket, &sender).await;
                    }
                    MessageType::StoreValueRequest => {
                        let (_, msg) = StoreValueRequestMessage::from_bytes(&buffer);
                        if msg.data.len() == 0 {
                            //TODO: reject?
                            return;
                        }
                        assert_eq!(msg.key.len(), 64);

                        //Am I closest to the key?
                        {
                            let route_table = cloned_route_table.lock().await;
                            if route_table.is_closest_to(&msg.key) {
                                //yes, save data on local
                                let cfh = cloned_kvdb.cf_handle(DHT_DATA_COLUMN_FAMILY).unwrap();
                                cloned_kvdb.put_cf(cfh, &msg.key, &msg.data).expect(
                                    "Failed to save a store request data on kvdb (put failed)",
                                );
                                return;
                            }
                        }

                        event!(Level::DEBUG, "Foward a store value request message");
                        let foward_count = utility::calculate_foward_count(
                            1000,
                            77, /*dummy hopcount TODO*/
                            msg.replication_level,
                        ); //TODO implement network size estimate

                        //find a node which is closest to the key
                        let nodes_to_foward;
                        {
                            let route_table = cloned_route_table.lock().await;
                            nodes_to_foward = route_table.find_nodes(&msg.key, foward_count.into());
                        }

                        if nodes_to_foward.len() == 0 {
                            //could not find
                            //TODO: handle, but what to do?
                            //TODO print hop count
                            event!(Level::ERROR, "Could not find closest peer");
                            return;
                        }
                        //TODO: modify hop count and etc here if needed
                        for node in &nodes_to_foward {
                            let ep;
                            {
                                let node = node.lock().unwrap();
                                ep = node.endpoint;
                            }
                            let wrriten_size = cloned_socket
                                .send_to(&msg.to_bytes(), ep)
                                .await
                                .expect("Failed to forward a store request");
                            assert_eq!(wrriten_size, buffer.len());
                        }
                    }
                    MessageType::FindNodeRequest => {
                        //TODO when to forward the messsage?
                        let (_, msg) = FindNodeRequestMessage::from_bytes(&buffer);
                        //   reject malformed messages

                        let nodes;
                        {
                            let route_table = cloned_route_table.lock().await;
                            nodes = route_table.find_nodes(&msg.key, 20); //todo set 'K'
                            event!(Level::ERROR, "TODO set k");
                        }
                        if nodes.len() == 0 {
                            //TODO: do something
                            event!(Level::DEBUG, "Closest peer not found");
                            return;
                        }
                        //TODO implement message and return
                        let mut addrs = Vec::with_capacity(nodes.len());
                        for node in &nodes {
                            let node = node.lock().unwrap();
                            addrs.push(node.endpoint);
                        }
                        let msg = FindNodeResponseMessage::new(&addrs);
                        let wrriten_size = cloned_socket
                            .send_to(&msg.to_bytes(), &sender)
                            .await
                            .expect("Failed to send find node response");
                        assert_eq!(wrriten_size, buffer.len());
                    }
                    MessageType::FindValueRequest => {
                        event!(Level::DEBUG, "Received find value request");

                        let (header, msg) = FindValueRequestMessage::from_bytes(&buffer);
                        debug_assert_eq!(header.message_type, MessageType::FindValueRequest as u32);
                        debug_assert_ne!(msg.key.len(), 0);

                        //check kvdb
                        let get_opt;
                        {
                            let cfh = cloned_kvdb.cf_handle(DHT_DATA_COLUMN_FAMILY).unwrap();
                            get_opt = cloned_kvdb
                                .get_cf(cfh, &msg.key)
                                .expect("Failed to perform kvdb get operation");
                        }
                        if get_opt.is_some() {
                            //value with the key found in (local) kvdb
                            let value = get_opt.unwrap();
                            let reply_msg =
                                FindValueResponseMessage::new(&msg.key, None, Some(&value));
                            let wrriten_size = cloned_socket
                                .send_to(&reply_msg.to_bytes(), sender)
                                .await
                                .expect("Failed to send a find value response (with value)");
                            assert_eq!(wrriten_size, reply_msg.to_bytes().len());
                            return;
                        }

                        assert!(get_opt.is_none());
                        //value with the key not found in local,
                        //reply with a closest node to the key

                        let nodes;
                        {
                            let route_table = cloned_route_table.lock().await;
                            nodes = route_table.find_nodes(&msg.key, 1);
                        }
                        //only ask to a closest peer
                        //plain implementation
                        //TODO: customize this
                        if nodes.len() == 0 {
                            //peer not found
                            //TODO something
                            event!(Level::DEBUG, "Closest peer not found");
                            return;
                        }
                        assert!(nodes.len() == 1);
                        let node = &nodes[0];
                        let response_msg;
                        {
                            let node = node.lock().unwrap();
                            response_msg =
                                FindValueResponseMessage::new(&msg.key, Some(&node.endpoint), None);
                        }
                        let wrriten_size = cloned_socket
                            .send_to(&response_msg.to_bytes(), sender)
                            .await
                            .expect("Failed to send a find value response (with node)");
                        assert_eq!(wrriten_size, response_msg.to_bytes().len());
                    }
                    MessageType::PingResponse => {
                        event!(Level::DEBUG, "Received a ping response from {}", &sender);

                        {
                            {
                                let mut ping_list = cloned_ping_list.lock().unwrap();
                                if !ping_list.contains(&sender) {
                                    //I have not pinged the sender, malicious
                                    //TODO: block the sender(not permanently)
                                    event!(
                                        Level::DEBUG,
                                        "Sender ({}) is not in the ping list",
                                        sender
                                    );

                                    return;
                                }
                                //remove the sender from ping list
                                assert!(ping_list.contains(&sender));
                                if !ping_list.remove(&sender) {
                                    panic!("Logic error");
                                }
                            }
                            event!(Level::DEBUG, "removed the sender from ping list");

                            let mut rt = cloned_route_table.lock().await;

                            let is_handled = rt.add_node(&sender).unwrap();

                            if !is_handled {
                                event!(Level::DEBUG, "Space not available for the new node");
                                let bucket = rt.find_bucket(&endpoint_to_node_id(&sender));
                                for node in &bucket.nodes {
                                    let ep;
                                    {
                                        let node = node.lock().unwrap();
                                        ep = node.endpoint;
                                    }
                                    {
                                        let mut ping_list = cloned_ping_list.lock().unwrap();
                                        //insert to ping list
                                        ping_list.insert(ep);
                                    }
                                    do_ping_impl(&cloned_socket, &ep).await;
                                }

                                //todo
                                event!(
                                    Level::WARN,
                                    "TODO: remove dead nodes from the route table and add new node"
                                );
                                return;
                            }
                        }
                        event!(Level::DEBUG, "add node");
                    }
                    MessageType::FindNodeResponse => {
                        //TODO: did I sent request?
                        //deserialize message
                        let (_, msg) = FindNodeResponseMessage::from_bytes(&buffer);

                        event!(
                            Level::DEBUG,
                            "Received find node response from {}. Contains {} nodes)",
                            &sender,
                            msg.nodes.len()
                        );

                        for n in &msg.nodes {
                            {
                                let mut route_table = cloned_route_table.lock().await;
                                let is_handled = route_table.add_node(n).unwrap();
                                //todo handle branch if route table(bucket) is full
                            }
                        }
                    }
                    MessageType::FindValueResponse => {
                        //TODO check if I actually requested the data
                        event!(
                            Level::DEBUG,
                            "Received find value response from {}",
                            &sender
                        );
                        let (_, msg) = FindValueResponseMessage::from_bytes(&buffer);
                        if msg.data.is_some() && msg.node.is_some() {
                            //malformed
                            //TODO maybe block the sender
                            return;
                        }
                        if msg.data.is_some() {
                            //save data
                            let cfh = cloned_kvdb.cf_handle(DHT_DATA_COLUMN_FAMILY).unwrap();
                            cloned_kvdb
                                .put_cf(cfh, &msg.key, &msg.data.unwrap())
                                .unwrap();
                            return;
                        }
                        if msg.node.is_some() {
                            //TODO
                            //maybe disable this feature for privacy reasons
                            event!(Level::ERROR, "TODO");
                            return;
                        }
                    }
                    _ => {
                        unreachable!();
                    }
                };
            }
        });
    }

    /// Check whether there is value with the given key on kvdb or not.
    pub fn is_available_on_local(&self, key: &[u8]) -> anyhow::Result<bool> {
        let cfh = self.kvdb.cf_handle(DHT_DATA_COLUMN_FAMILY).unwrap();

        let opt = self.kvdb.get_cf(&cfh, &key)?;
        if opt.is_some() {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Initiate a ping request.
    pub async fn do_ping(&self, endpoint: &SocketAddr) -> Result<()> {
        {
            //insert to ping list
            let mut ping_list = match self.ping_list.lock() {
                Ok(pl) => pl,
                Err(e) => {
                    return Err(anyhow!(e.to_string()));
                }
            };
            ping_list.insert(*endpoint);
            event!(Level::DEBUG, "Inserted {} to the ping list", endpoint);
        }
        do_ping_impl(&self.udp_socket, endpoint).await?;
        Ok(())
    }

    // TODO: maybe return Result<bool>
    // TODO: maybe only accept (key, data) such that key == hash(data)
    /// Store a value(data) at the given key on network.
    /// This function will not store the given data locally.
    pub async fn do_store(&self, key: &[u8], data: &[u8]) -> Result<()> {
        let request_msg = StoreValueRequestMessage::new(key, data, 10); //todo implement replication level
        let tmp = 10; //TODO implement
        let nodes_to_foward;
        {
            let route_table = self.route_table.lock().await;
            nodes_to_foward = route_table.find_nodes(key, tmp);
        }
        if nodes_to_foward.len() == 0 {
            //TODO: do something
            return Err(anyhow!("Could not find peers to foward"));
        }
        for node in &nodes_to_foward {
            let ep;
            {
                let node = node.lock().unwrap();
                ep = node.endpoint;
            }
            self.udp_socket
                .send_to(&request_msg.to_bytes(), ep)
                .await
                .expect("Failed to send a store request");
        }
        Ok(())
    }

    /// Initiate a find value request.
    pub async fn do_find_value(&self, key: &[u8]) -> Result<()> {
        let request_msg = FindValueRequestMessage::new(key);
        //check local first
        let cfh = self.kvdb.cf_handle(DHT_DATA_COLUMN_FAMILY).unwrap();
        let opt = self.kvdb.get_cf(cfh, key)?;

        if opt.is_some() {
            //found on local
            event!(
                Level::DEBUG,
                "value for the key {} is found on the local kvdb",
                hex::encode(key)
            );
            return Ok(());
        }
        //not found, ask to peers
        event!(
            Level::DEBUG,
            "value for the key {} is not found on the local kvdb",
            hex::encode(key)
        );
        let tmp = 10; //TODO implement
        let nodes_to_foward;
        {
            let route_table = self.route_table.lock().await;
            nodes_to_foward = route_table.find_nodes(key, tmp);
        }
        if nodes_to_foward.len() == 0 {
            //TODO: do something
            return Err(anyhow!("Could not find peers to foward"));
        }
        for node in &nodes_to_foward {
            let node = node.lock().unwrap();
            debug_assert_ne!(request_msg.key.len(), 0);
            let _ = self
                .udp_socket
                .send_to(&request_msg.to_bytes(), &node.endpoint)
                .await?;
        }
        Ok(())
    }

    /// Initiate a find node request.
    pub async fn do_find_node(&self, key: &[u8]) -> Result<()> {
        let request_msg = FindNodeRequestMessage::new(key);
        let peers;
        {
            let route_table = self.route_table.lock().await;
            peers = route_table.find_nodes(key, 100); //TODO change 100 to reasonable value
        }
        if peers.len() == 0 {
            event!(
                Level::DEBUG,
                "Could not find peers to send find node request"
            );
        }

        for peer in &peers {
            let peer = peer.lock().unwrap();
            let _ = self
                .udp_socket
                .send_to(&request_msg.to_bytes(), &peer.endpoint)
                .await?;
        }
        Ok(())
    }

    /// Store a value to kvdb.
    pub fn store_on_local(&self, key: &[u8], data: &[u8]) -> Result<()> {
        let cfh = self.kvdb.cf_handle(DHT_DATA_COLUMN_FAMILY).unwrap();
        self.kvdb.put_cf(cfh, key, data)?;
        Ok(())
    }

    /// Get value with the given key from kvdb
    /// Returns Ok(None) if not found
    pub fn get_value_local(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let cfh = self.kvdb.cf_handle(DHT_DATA_COLUMN_FAMILY).unwrap();
        self.kvdb
            .get_cf(cfh, key)
            .map_err(|e| anyhow::Error::from(e))
    }

    /* dht-dev features */
    /// Convenience function for cocoon-virtual.
    #[cfg(feature = "dht-dev")]
    pub fn local_endpoint(&self) -> Result<SocketAddr> {
        self.udp_socket
            .local_addr()
            .map_err(|e| anyhow::Error::from(e))
    }
}

async fn do_ping_impl(udp_socket: &Arc<UdpSocket>, endpoint: &SocketAddr) -> Result<()> {
    let msg = PingRequestMessage::new();
    udp_socket.send_to(&msg.to_bytes(), endpoint).await?;

    event!(Level::DEBUG, "Sent a ping message to {}", &endpoint);
    Ok(())
}

//send ping reply
async fn pong(udp_socket: &UdpSocket, endpoint: &SocketAddr) -> Result<()> {
    let msg = PingResponseMessage::new();
    udp_socket.send_to(&msg.to_bytes(), endpoint).await?;
    event!(Level::DEBUG, "Sent pong message to {}", &endpoint);
    Ok(())
}
