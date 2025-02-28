use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use tonic::transport::{Channel, Endpoint};

use raft::{RawNode, eraftpb, storage::{Storage, MemStorage}};
use raft::eraftpb::Message;
use raft::prelude::Ready;
use raft::GetEntriesContext;

use crate::core::raft::Operation;
use crate::core::raft::rpc::proto::{ raft_server::Raft, raft_client::RaftClient};
use crate::core::raft::rpc::proto::{
    RequestVoteRequest, RequestVoteResponse,
    AppendEntriesRequest, AppendEntriesResponse,
    InstallSnapshotRequest, InstallSnapshotResponse,
};


use crate::core::raft::cluster_config;
use crate::core::counter::Counter;

pub struct RaftService {
    node: Arc<Mutex<RawNode<MemStorage>>>,
    node_addresses: HashMap<u64, String>, //ID to address mapping
    state_machine: Mutex<Counter>,
    node_channels: Mutex<HashMap<String, Channel>>,
}

impl RaftService {
    pub fn new(node: Arc<Mutex<RawNode<MemStorage>>>, node_addresses: HashMap<u64, String>) -> Self {
        RaftService {
            node,
            node_addresses,
            state_machine: Mutex::new(Counter::new(0)),
            node_channels: Mutex::new(HashMap::new()),
        }
    }

    async fn process_ready(&self,  ready: &Ready) -> Result<Response<RequestVoteResponse>, Status> {
        self.send_raft_messages(&ready.messages()).await?;
        self.apply_committed_entries(&ready.committed_entries()).await?;

        // Dummy Response
        let reply_msg = eraftpb::Message::default();
        let res = RequestVoteResponse {
            message: Some(reply_msg),
        };
        Ok(Response::new(res))
    }

    async fn send_raft_messages(&self, messages: &[eraftpb::Message])-> Result<(), Status> {
        for message in messages {
            tracing::debug!("Sending message: {:?}", message);
            if message.to == 0 {
                tracing::warn!("Message to node id 0, skipping");
                continue;
            }
            if let Some(address) = self.node_addresses.get(&message.get_to()) {
                let channel = self.get_grpc_channel(address).await?; // Use cached channel
                let mut raft_client = RaftClient::new(channel);

                match message.get_msg_type() {
                    eraftpb::MessageType::MsgRequestVote => {
                        let response = self.send_request_vote(&mut raft_client, message.clone()).await?;
                        if let Some(msg) = response.message {
                            if msg.get_reject() == false {
                                tracing::debug!("Vote granted by node: {}", message.get_to());
                                // Update vote tracking if needed
                            }
                            let mut node = self.node.lock().await;
                            if msg.get_term() > node.raft.term {
                                tracing::debug!("Term update from response vote");
                                node.raft.term = msg.get_term();
                                //self.update_term(&mut node, msg.get_term()).await;
                            }
                        }
                    }
                    eraftpb::MessageType::MsgAppend => {
                        let response = self.send_append_entries(&mut raft_client, message.clone()).await?;
                        if let Some(msg) = response.message {
                            if msg.get_reject() == false {
                                tracing::debug!("Append entries success to node: {}", message.get_to());
                                let mut node = self.node.lock().await;
                                //node.raft.report_snapshot(msg.get_to(), msg.get_index());
                                if msg.get_term() > node.raft.term {
                                    tracing::debug!("Term update from Append response:  updated to {}", msg.get_term());
                                    node.raft.term = msg.get_term();
                                    //self.update_term(&mut node, msg.get_term()).await;
                                }
                            } else {
                                tracing::debug!("Append entries reject from node: {}", message.get_to());
                                self.handle_log_inconsistency(message.get_to(), msg.get_index(), msg.get_log_term()).await;
                            }
                        }
                    }
                    eraftpb::MessageType::MsgSnapshot => {
                        let response = self.send_install_snapshot(&mut raft_client, message.clone()).await?;
                        if let Some(msg) = response.message {
                            let mut node = self.node.lock().await;
                            if msg.get_term() > node.raft.term {
                                tracing::debug!("Term update from snapshot response");
                                node.raft.term = msg.get_term();
                                //self.update_term(&mut node, msg.get_term()).await;
                            }
                        }
                    }
                    _ => {tracing::warn!("Message type not handled: {:?}", message.get_msg_type());
                    }
                }
            } else {
                tracing::error!("Node address not found for node id: {:?}", message.get_to());
            }
        }
        Ok(())
    }

    async fn handle_log_inconsistency(&self, node_id: u64, follower_index: u64, follower_log_term: u64) {
        let mut node = self.node.lock().await;
        // 1. Adjust next_index
        let next_index = follower_index; // Adjust based on follower's information

        // 2. Construct new MsgAppend message
        let mut new_message = eraftpb::Message::default();
        new_message.set_msg_type(eraftpb::MessageType::MsgAppend);
        new_message.set_to(node_id);
        new_message.set_from(node.raft.id);
        new_message.set_term(node.raft.term);

        // 3. Add correct log entries
        let raft_log = &node.raft.raft_log;
        let context = GetEntriesContext::empty(true);
        match raft_log.entries(next_index, raft_log.last_index() + 1, context) {
            Ok(entries) => new_message.set_entries(entries.into()),
            Err(e) => {
                tracing::error!("Error retrieving log entries: {:?}", e);
                //what should I do here? panic ?
                panic!("{}", format!("Error retrieving log entries: {:?}", e));
            }
        }

        // 4. Call step()
        node.raft.step(new_message).unwrap();

        // 5. Handle Ready
        let ready = node.ready();
        drop(node);
        self.process_ready(&ready);
    }

    async fn get_grpc_channel(&self, address: &str) -> Result<Channel, Status> {
        let mut channels = self.node_channels.lock().await;
        if let Some(channel) = channels.get(address) {
            Ok(channel.clone())
        } else {
            let channel = Endpoint::from_shared(address.to_string())
                .map_err(|e| Status::internal(format!("Error creating endpoint: {:?}", e)))?
                .connect()
                .await
                .map_err(|e| Status::internal(format!("Error connecting to endpoint: {:?}", e)))?;
            channels.insert(address.to_string(), channel.clone());
            Ok(channel)
        }
    }
    async fn send_request_vote(&self, client: &mut RaftClient<Channel>, message: eraftpb::Message) -> Result<RequestVoteResponse, Status> {
        let vote_request = RequestVoteRequest { message: Some(message.clone()) };
        let response = client.request_vote(vote_request).await
            .map(|res| res.into_inner())
            .map_err(|e| Status::internal(format!("Error sending RequestVote: {:?}", e)));

        tracing::debug!("response received: {:?}", response);
        Ok(response?)
    }

    async fn send_append_entries(&self, client: &mut RaftClient<Channel>, message: eraftpb::Message) -> Result<AppendEntriesResponse, Status> {
        let append_request = AppendEntriesRequest { message: Some(message.clone()), entries: vec![] }; // Adjust Entries
        let response = client.append_entries(append_request).await
            .map(|res| res.into_inner())
            .map_err(|e| Status::internal(format!("Error sending AppendEntries: {:?}", e)));

        tracing::debug!("response received: {:?}", response);
        Ok(response?)
    }

    async fn send_install_snapshot(&self, client: &mut RaftClient<Channel>, message: eraftpb::Message) -> Result<InstallSnapshotResponse, Status> {
        let snapshot_request = InstallSnapshotRequest { message: Some(message.clone()) };
        let response = client.install_snapshot(snapshot_request).await
            .map(|res| res.into_inner())
            .map_err(|e| Status::internal(format!("Error sending InstallSnapshot: {:?}", e)));

        tracing::debug!("response received: {:?}", response);
        Ok(response?)
    }

    async fn update_term(&mut self, node: &mut RawNode<MemStorage>, term: u64) {
        node.raft.term = term;
        tracing::debug!("Term updated to {}", term);
    }

    //async fn update_match_index(&mut self, node_id: u64, match_index: u64) {
        //let mut node = self.node.lock().await;
        //let raft = &mut node.raft;

        //raft.prs.get_mut(&node_id).unwrap().match_idx = match_index;
        //raft.prs.get_mut(&node_id).unwrap().next_idx = match_index + 1;
    //}

    //async fn adjust_next_index(&mut self, node_id: u64, next_index: u64) {
        //let mut node = self.node.lock().await;
        //let raft = &mut node.as_mut().unwrap().raft.raft;
        //raft.prs.get_mut(&node_id).unwrap().next_idx = next_index;
    //}

    async fn apply_committed_entries(&self, entries: &[eraftpb::Entry])-> Result<(), Status> {
        for entry in entries {
            tracing::debug!("Applying committed entry: {:?}", entry);
            if entry.data.is_empty() {
                continue;
            }

            match serde_json::from_slice::<Operation>(&entry.data) {
                Ok(op) => {
                    tracing::debug!("Operation: {:?}", op);
                    // TODO: Investigate why deserialization produces &u64 and fix the root cause.
                    let op_clone = op.clone();

                    let mut state = self.state_machine.lock().await;
                    if let Err(e) = state.apply_operation(op_clone) {
                        tracing::error!("Error applying operation {:?}", op);
                        return Err(Status::internal(format!("Error applying operation {:?}", e)));
                    }
                }
                Err(e) => {
                    tracing::error!("Error decoding entry data: {:?}", e);
                    return Err(Status::internal(format!("Error decoding entry data: {:?}", e)));
                }
            }

            // Update applied index
            //let mut node = self.node.lock().await;
            //if let Err(e) = node.apply_conf_change(&eraftpb::ConfChange::default()){
                    //tracing::error!("Error applying conf change: {:?}", e);
                    //return Err(Status::internal(format!("Error applying conf change: {:?}", e)));
            //}; // TODO applying default ... this will change
        }
        Ok(())
    }
}

#[tonic::async_trait]
impl Raft for RaftService {

    async fn request_vote(&self, request: Request<RequestVoteRequest>) -> Result<Response<RequestVoteResponse>, Status> {
        let req = request.into_inner();
        tracing::debug!("Got request_vote request: {:?}", req);

        match req.message {
            Some(eraft_msg) => {
                let mut node = self.node.lock().await;
                let response = handle_request_vote(&node, &eraft_msg);

                if let Err(e) = node.step(eraft_msg) {
                    tracing::error!("Failed to step raft node: {:?}", e);
                    return Err(Status::internal(format!("Raft node step failed: {:?}", e)));
                }

                let ready = node.ready();

                if let Err(e) = self.process_ready(&ready).await {
                    tracing::error!("Failed to procss ready: {:?}", e);
                    return Err(Status::internal(format!("Failed to process ready: {:?}", e)))
                }

                node.advance(ready);

                tracing::debug!("Sending RequestVoteResponse: {:?}", response);
                Ok(Response::new(RequestVoteResponse{ message: Some(response)}))
            }
            None => {
                tracing::error!("RequestVoteRequest message is missing");
                return Err(Status::invalid_argument("RequestVoteRequest message is missing"));
            }
        }
    }


    async fn append_entries(&self, request: Request<AppendEntriesRequest>) -> Result<Response<AppendEntriesResponse>, Status> {
        let req = request.into_inner();
        tracing::debug!("Got append_entries request: {:?}", req);

        match req.message {
            Some(eraft_msg) => {
                let mut node = self.node.lock().await;

                if let Some(response) = check_log_inconsistency(&node, &eraft_msg) {
                    tracing::error!("Foind log inconsistency! Sending rejection: {:?}", response);
                    return Ok(Response::new(AppendEntriesResponse{message: Some(response)})); // Early rejection
                }

                if let Err(e) = node.step(eraft_msg.clone()){
                    tracing::error!("Failed to step raft node: {:?}", e);
                    return Err(Status::internal(format!("Raft node step failed: {:?}", e)));
                }

                let ready = node.ready();

                if let Some(response) = handle_ready_rejections(&node, &ready, &eraft_msg) {
                    return Ok(Response::new(AppendEntriesResponse{message: Some(response)})); // Rejection after step()
                }

                let response = construct_append_response(&node, &eraft_msg);

                if let Err(e) = self.process_ready(&ready).await {
                    tracing::error!("Failed to procss ready: {:?}", e);
                    return Err(Status::internal(format!("Failed to process ready: {:?}", e)))
                }

                Ok(Response::new(AppendEntriesResponse{message: response})) // Rejection after step()
            }
            None => {
               tracing::error!("RequestVoteRequest message is missing");
               return Err(Status::invalid_argument("RequestVoteRequest message is missing"));
            }
        }
    }

    async fn install_snapshot(&self, request: Request<InstallSnapshotRequest>) -> Result<Response<InstallSnapshotResponse>, Status> {

        let req = request.into_inner();
        tracing::debug!("Got install_snapshot request: {:?}", req);

        match req.message {
            Some(eraft_msg) => {
                let mut node = self.node.lock().await;

                if let Err(e) = node.step(eraft_msg.clone()){
                    tracing::error!("Failed to step raft node: {:?}", e);
                    return Err(Status::internal(format!("Raft node step failed: {:?}", e)));
                }

                let ready = node.ready();
                if let Some(error_response) = handle_ready_snapshot_errors(&node, &ready, &eraft_msg) {
                   return Ok(Response::new(InstallSnapshotResponse{message: Some(error_response)}));
                }

                if let Err(e) = self.process_ready(&ready).await {
                    tracing::error!("Failed to procss ready: {:?}", e);
                    return Err(Status::internal(format!("Failed to process ready: {:?}", e)));
                }

                //WE are ignoring this anyway !!
                let response = Message {
                    from: node.raft.id,
                    to: eraft_msg.from,
                    term: node.raft.term,
                    msg_type: eraftpb::MessageType::MsgAppendResponse.into(),
                    reject: false,
                    ..Default::default()
                };
                Ok(Response::new(InstallSnapshotResponse{message: Some(response)}))
            },
            None => {
                tracing::error!("InstallSnapshot message is missing");
                return Err(Status::invalid_argument("InstallSnapshot message is missing"));
            }
        }
    }
}

fn handle_request_vote(node: &RawNode<MemStorage>, msg: &Message) -> Message {
    let raft = &node.raft;
    let last_log_index = raft.raft_log.last_index();
    let last_log_term = raft.raft_log.last_term();

    let mut response = Message {
        from: raft.id,
        to: msg.from,
        term: raft.term,
        msg_type: eraftpb::MessageType::MsgRequestVoteResponse.into(),
        reject: false, // Assume vote granted initially
        ..Default::default()
    };

    // 1. Higher Term Check
    if msg.term < raft.term {
        response.reject = true;
        return response;
    }

    // 2. Already Voted Check
    //      voted            to someon else        in this term
    if raft.vote != 0 && raft.vote != msg.from && raft.term == msg.term {
        response.reject = true;
        return response;
    }

    // 3. Log Up-to-Dateness Check
    if msg.log_term < last_log_term
        || (msg.log_term == last_log_term && msg.index < last_log_index)
    {
        response.reject = true;
        return response;
    }

    // Vote Granted
    response.term = msg.term; // Update the term, if it is higher
    response
}

fn check_log_inconsistency(node: &RawNode<MemStorage>, msg: &Message) -> Option<Message> {
    let raft = &node.raft;
    let last_index = raft.raft_log.last_index();

    let context = GetEntriesContext::empty(false);

    if msg.index - 1 > 0 { // prev idx > 0 => not the first
        if last_index < msg.index - 1 { // last log idx < prev idx => missing entries
            return Some(create_reject_response(node, msg, last_index + 1));
        } else if let Ok(entry) = raft.raft_log.entries(msg.index -1, msg.index +1, context) {
            if entry.last().unwrap().term != msg.term -1 { // already have a message from same term
                return Some(create_reject_response(node, msg, msg.index -1));
            }
        } else {
            return Some(create_reject_response(node, msg, msg.index -1));
        }
    }
    None // No inconsistency detected
}

fn handle_ready_rejections(node: &RawNode<MemStorage>, ready: &Ready, msg: &Message) -> Option<Message> {
    // What cases are to be covered here ?
    if !ready.entries().is_empty() {
        for entry in ready.entries() {
            if entry.get_data().is_empty() { //Example error condition
                let raft = &node.raft;
                return Some(create_reject_response(&node, msg, entry.index));
            }
        }
    }
    None
}

fn create_reject_response(node: &RawNode<MemStorage>, msg: &Message, reject_hint: u64) -> Message {
    Message {
        from: node.raft.id,
        to: msg.from,
        term: node.raft.term,
        msg_type: eraftpb::MessageType::MsgAppendResponse.into(),
        reject: true,
        reject_hint,
        ..Default::default()
    }
}

// Function to construct the MsgAppendResponse (success case)
fn construct_append_response(node: &RawNode<MemStorage>, msg: &Message) -> Option<Message> {
    let raft = &node.raft;
    let last_index = raft.raft_log.last_index();

    let response = Message {
        from: raft.id,
        to: msg.from,
        term: raft.term,
        msg_type: eraftpb::MessageType::MsgAppendResponse.into(),
        index: last_index,
        reject: false,
        ..Default::default()
    };
    Some(response)
}

fn handle_ready_snapshot_errors(node: &RawNode<MemStorage>, ready: &Ready, msg: &Message) -> Option<Message> {
    if !ready.snapshot().is_empty() {
        if ready.snapshot().get_metadata().get_index() == 0 { //Example error condition
            tracing::error!("Snapshot metadata index is invalid.");
            return Some(create_snapshot_error_response(node, msg));
        }

        // Example: Check for other error conditions (e.g., corrupted data)
        if ready.snapshot().get_data().is_empty() { //Example error condition
            tracing::error!("Error: Snapshot data is empty.");
            return Some(create_snapshot_error_response(node, msg));
        }

        // Add more error checks here!.
    }
    None
}

// Function to create a snapshot error response
fn create_snapshot_error_response(node: &RawNode<MemStorage>, msg: &Message,) -> Message {
    Message {
        from: node.raft.id,
        to: msg.from,
        term: node.raft.term,
        msg_type: eraftpb::MessageType::MsgAppendResponse.into(), // Or use a custom error type
        reject: true,
        reject_hint: 0, // Indicate an error occurred
        ..Default::default()
    }
}


#[cfg(test)]
mod tests {
    //use raft::prelude::Ready;
    //use crate::create_raft_service;
    use super::*;
    use raft::eraftpb;
    use serde_json;
    use tokio::sync::Mutex;

    fn setup_raft_service() -> RaftService {
        let mut config = raft::Config::default();
        config.id = 1;
        let conf_state = eraftpb::ConfState {
            voters: vec![1], // Add node ID 1 as a voter
            ..Default::default()
        };

        let storage = raft::storage::MemStorage::new();
        storage.initialize_with_conf_state(conf_state);


        let state_machine = Mutex::new(Counter::new(0));
        let raw_node = Mutex::new(
            raft::RawNode::new(
                &config,
                storage,
                &slog::Logger::root(
                    slog::Discard,
                    slog::o!()))
            .unwrap());

        RaftService {
            state_machine,
            node: std::sync::Arc::new(raw_node),
            node_addresses: std::collections::HashMap::new(),
            node_channels: Mutex::new(HashMap::new()),
        }
    }

    #[tokio::test]
    async fn test_apply_committed_entries() {
        let service = setup_raft_service();

        let op1    = Operation::IncrementBy(10);
        let data1  = serde_json::to_vec(&op1).unwrap();
        let entry1 = eraftpb::Entry {
            data: data1,
            ..Default::default()
        };

        let op2    = Operation::DecrementBy(5);
        let data2  = serde_json::to_vec(&op2).unwrap();
        let entry2 = eraftpb::Entry {
            data: data2,
            ..Default::default()
        };

        let entries = vec![entry1, entry2];
        service.apply_committed_entries(&entries).await.unwrap();

        let state = service.state_machine.lock().await;
        assert_eq!(state.value, 5);
    }
}


