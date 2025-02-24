use tracing::{info, debug, instrument};
use tracing_slog::TracingSlogDrain;
use slog::{Logger, Drain};

use std::collections::HashMap;
use std::sync::Arc;

use crate::core::raft::rpc::proto::raft_server::RaftServer;
use crate::core::raft::service::RaftService;
use crate::core::raft::cluster_config::{get_cluster_config, ClusterConfig};

use raft::{Config, RawNode, Ready};
use raft::eraftpb::{Entry, EntryType, Message, MessageType};
use raft::storage::MemStorage;

use tokio::sync::Mutex;
use tracing_test::traced_test;

use super::operation::Operation;
use super::state::RaftState;

//use slog::Logger;
//use slog_term::TermDecorator;
//use slog::Drain;

#[instrument(name = "new_config", level = "debug")]
fn new_config (node_id: u64) -> Config {
    info!("Creating new Raft config for node {}", node_id);
    Config {
        id: node_id, // Unique ID for this node
        election_tick: 10,
        heartbeat_tick: 1,
        applied: 0,
        max_size_per_msg: 1024 * 1024,
        max_inflight_msgs: 256,
        ..Default::default()
    }
}

#[instrument(name = "create_raft_node", level = "info")]
fn create_raft_node(node_id: u64) -> Result<Arc<Mutex<RawNode<MemStorage>>>, raft::Error> {

    info!("Creating Raft node with ID: {}", node_id);
    let config = new_config(node_id);
    let node   = create_raft_rawnode(&config)?;

    info!("Raft node created successfully.");
    Ok(node)
}

#[instrument(name = "create_raft_service", level = "info")]
pub fn create_raft_service(node_id: u64) -> Result<RaftService, raft::Error> {
    info!("Creating Raft service with node ID: {}", node_id);
    let node    = create_raft_node(node_id)?;
    let config  = get_cluster_config();
    let service = RaftService::new(node, config.node_addresses);
    info!("Raft service created successfully.");
    Ok(service)
}

#[instrument(name = "create_raft_rawnode", level = "info", skip(config))]
fn create_raft_rawnode(config: &Config) -> Result<Arc<Mutex<RawNode<MemStorage>>>, raft::Error> {
    info!("Creating Raft RawNode...");

    // Initialize a Raft state store.
    let storage   = MemStorage::new();

    let drain  = TracingSlogDrain.filter_level(slog::Level::Debug);
    let logger = Logger::root(drain.fuse(), slog::o!());

    //let decorator = TermDecorator::new().stderr().build();
    //let drain     = slog_term::FullFormat::new(decorator).build().fuse();
    //let drain     = slog_async::Async::new(drain).build().fuse();
    //let logger    = Logger::root(drain, slog::o!());

    // Create a Raft node.
    let node = RawNode::new(config, storage, &logger)?;
    info!("Raft RawNode created.");

    let node = Arc::new(Mutex::new(node));

    Ok(node)
}
