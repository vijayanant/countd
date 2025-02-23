use tracing::{info, debug, instrument};


use std::sync::Arc;
use tokio::sync::Mutex;

use raft::{Config, RawNode, Ready};
use raft::eraftpb::{Entry, EntryType, Message, MessageType};
use raft::storage::MemStorage;
use tokio::sync::mpsc;
use tracing_test::traced_test;

use super::command::Command;
use super::state::RaftState;

use slog::Logger;
use slog_term::TermDecorator;
use slog::Drain;

pub async fn new_config (id: u64) -> Config {
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
pub async fn start_raft(config: &Config) -> (Arc<Mutex<RawNode<MemStorage>>>, mpsc::Sender<Message>) {
    info!("Starting Raft service...");

    // Initialize a Raft state store.
    let storage = MemStorage::new();

    // Create a slog logger
    let decorator = TermDecorator::new().stderr().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let logger = Logger::root(drain, slog::o!());


    let (tx_raft, mut rx_raft) = mpsc::channel::<Message>(100); // For Raft messages to RawNode
    let (tx_network, mut rx_network) = mpsc::channel::<Message>(100); // For RawNode messages to network


    // Create a Raft node.
    let node = RawNode::new(config, storage, &logger).unwrap();

    //we cant move the node into the tokio::spawn closure and also
    //return it from the start_raft() function. We will wrap the  RawNode
    //in an Arc<Mutex<>> , and then  we can clone it and move the clone into
    //the tokio::spawn closure. The original will be returned from start_raft().
    let node = Arc::new(Mutex::new(node));
    let node_clone = node.clone(); // Clone

    // Raft message receiver task
    tokio::spawn(async move {

        //#[instrument(fields(message_type = ?msg.msg_type))]
        while let Some(msg) = rx_raft.recv().await {
            debug!("Received message: {:?}", msg); // Trace the received message

            let mut node  = node_clone.lock().await;
            node.step(msg).unwrap();
            let ready = node.ready();

            debug!("Ready state: {:?}", ready); // Trace the ready state
            if !ready.messages().is_empty() {
                for msg in ready.messages() {
                    // send messages to network task
                    tx_network.send(msg.clone()).await.unwrap();
                }
            }
            node.advance(ready);
        }
    });

    // network sender task
    let node_clone = node.clone(); //clone again!!
    tokio::spawn(async move {
        while let Some(msg) = rx_network.recv().await {
            debug!("Sending message (placeholder): {:?}", msg); // Trace message being sent
            let node = node_clone.lock().await;
            //eventually send the messages through grpc
        }
    });

    info!("Raft service started.");
    (node, tx_raft)  //return the raft sender along with rawNode
}


#[cfg(test)]
mod tests {
    use super::*;

    #[traced_test]
    #[tokio::test]
    async fn test_message_flow() {
        let config = new_config(1).await;
        let (node, tx) = start_raft(&config).await;

        //Propose a command
        let command = Command::Increment;

        let mut entry = Entry::default();
        entry.entry_type = EntryType::EntryNormal.into();
        entry.data = serde_json::to_vec(&command).unwrap().into();

        let mut message = Message::default();
        message.msg_type = MessageType::MsgPropose.into();
        message.from = 1;
        message.to = 1;
        message.entries = vec![entry].into();


        tx.send(message).await.unwrap();

        // Give some time for the messages to be processed
        let _ = tokio::time::sleep(std::time::Duration::from_millis(100));
    }

}
