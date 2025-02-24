use tracing::{info, debug, instrument};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::core::raft::rpc::proto::raft_server::RaftServer;
use crate::core::raft::service::RaftService;

use raft::{Config, RawNode, Ready};
use raft::eraftpb::{Entry, EntryType, Message, MessageType};
use raft::storage::MemStorage;
use tokio::sync::mpsc;
use tracing_test::traced_test;

use super::operation::Operation;
use super::state::RaftState;

use slog::Logger;
use slog_term::TermDecorator;
use slog::Drain;

fn new_config (id: u64) -> Config {
    Config {
        id: id, // Unique ID for this node
        election_tick: 10,
        heartbeat_tick: 1,
        applied: 0,
        max_size_per_msg: 1024 * 1024,
        max_inflight_msgs: 256,
        ..Default::default()
    }
}

fn create_raft_node(node_id: u64)
    -> (Arc<Mutex<RawNode<MemStorage>>>,
        HashMap<u64, String>) {

    let config = new_config(node_id);
    let node = create_raft_rawnode(&config);
    let addresses: HashMap<u64, String> = HashMap::new();

    (node, addresses)
}

pub fn create_raft_service(node_id: u64) -> RaftService {
    let (node, addresses) = create_raft_node(node_id);
    RaftService::new(node, addresses)
}

fn create_raft_rawnode(config: &Config) -> Arc<Mutex<RawNode<MemStorage>>> {
    info!("Starting Raft service...");

    // Initialize a Raft state store.
    let storage   = MemStorage::new();

    // Create a slog logger
    let decorator = TermDecorator::new().stderr().build();
    let drain     = slog_term::FullFormat::new(decorator).build().fuse();
    let drain     = slog_async::Async::new(drain).build().fuse();
    let logger    = Logger::root(drain, slog::o!());

    // Create a Raft node.
    let node = RawNode::new(config, storage, &logger).unwrap();

    let node = Arc::new(Mutex::new(node));

    node
}
