use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use tonic::transport::{Channel, Endpoint};

use raft::{RawNode, eraftpb, storage::MemStorage};
use raft::prelude::Ready;

use crate::core::raft::Operation;
use crate::core::raft::rpc::proto::{
    raft_server::Raft, raft_client::RaftClient,
    RequestVoteRequest, RequestVoteResponse,
    AppendEntriesRequest, AppendEntriesResponse,
    InstallSnapshotRequest, InstallSnapshotResponse,
};

use crate::core::counter::Counter;

pub struct RaftService {
    node: Arc<Mutex<RawNode<MemStorage>>>,
    node_addresses: HashMap<u64, String>, //ID to address mapping
    state_machine: Mutex<Counter>,
}

impl RaftService {
    pub fn new(node: Arc<Mutex<RawNode<MemStorage>>>, node_addresses: HashMap<u64, String>) -> Self {
        RaftService {
            node,
            node_addresses,
            state_machine: Mutex::new(Counter::new(0)),
        }
    }

    async fn process_ready(&self, ready: Ready) -> Result<Response<RequestVoteResponse>, Status> {
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

            if let Some(address) = self.node_addresses.get(&message.to) {
                match Endpoint::from_shared(address.clone()) {
                    Ok(endpoint) => {
                        match endpoint.connect().await {
                            Ok(channel) => {
                                let mut raft_client = RaftClient::new(channel);
                                let vote_request = RequestVoteRequest {message: Some(message.clone())};
                                match raft_client.request_vote(vote_request).await {
                                    Ok(response) => { tracing::debug!("response received: {:?}", response)}
                                    Err(e) => {tracing::error!("Eror sending message: {:?}",e)}
                                }
                            },
                            Err(e) => tracing::error!("error connecting to endpoint {:?}", e),
                        }
                    }
                    Err(e) => tracing::error!("error crating endpoint {:?}", e),
                }
            } else {
                tracing::error!("Node address not found for node id: {:?}", message.to);
            }
        }
        Ok(())
    }

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
            let mut node = self.node.lock().await;
            if let Err(e) = node.apply_conf_change(&eraftpb::ConfChange::default()){
                    tracing::error!("Error applying conf change: {:?}", e);
                    return Err(Status::internal(format!("Error applying conf change: {:?}", e)));
            }; // TODO applying default ... this will change
        }
        Ok(())
    }
}

#[tonic::async_trait]
impl Raft for RaftService {
    async fn request_vote(
        &self,
        request: Request<RequestVoteRequest>,
    ) -> Result<Response<RequestVoteResponse>, Status> {

        let req = request.into_inner();
        tracing::debug!("Got request_vote request: {:?}", req);

        match req.message {
            Some(eraft_msg) => {
                let step_result = {
                    let mut node = self.node.lock().await;
                    let _ = node.step(eraft_msg);
                };

                match step_result {
                    () => { // this stuppic Result<()> pttern, () matches everything !!
                        let ready = {
                            let mut node = self.node.lock().await;
                            node.ready()
                        };
                        self.process_ready(ready).await
                    }
                    //_  =>{  // thanks to (), no code reaches here !!
                        //tracing::error!("Failed to step raft node");
                        //return Err(Status::internal("Raft node step failed"));
                    //}
                }
            }
            None => {
                tracing::error!("RequestVoteRequest message is missing");
                return Err(Status::invalid_argument("RequestVoteRequest message is missing"));
            }
        }
    }




    async fn append_entries(
        &self,
        request: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {

        let req = request.into_inner();
        tracing::debug!("Got append_entries request: {:?}", req);

        match req.message {
            Some(eraft_msg) => {
                let ready = {
                    let mut node = self.node.lock().await;
                    node.step(eraft_msg)
                };
                match ready {
                    Ok(ready) => {
                        //TODO Process ready request
                        tracing::debug!("ready struct: {:?}", ready);
                        //dummy response
                        let res_msg = eraftpb::Message::default();
                        let res = AppendEntriesResponse {
                            message: Some(res_msg),
                        };
                        Ok(Response::new(res))
                    }
                    Err(e) => {
                        tracing::error!("Failed to step raft node: {:?}", e);
                        return Err(Status::internal("Raft node step failed"));
                    }
                }
            }
            None => {
                tracing::error!("AppendEntriesRequest message is missing");
                return Err(Status::invalid_argument("AppendEntriesRequest message is missing"));
            }
        }
    }

    async fn install_snapshot(
        &self,
        request: Request<InstallSnapshotRequest>,
    ) -> Result<Response<InstallSnapshotResponse>, Status> {

        let req = request.into_inner();
        tracing::debug!("Got install_snapshot request: {:?}", req);

        match req.message {
            Some(eraft_msg) => {
                let ready = {
                    let mut node = self.node.lock().await;
                    node.step(eraft_msg)
                };
                match ready {
                    Ok(ready) => {
                        // TODO: Process ready request
                        tracing::debug!("ready struct: {:?}", ready);

                        //dummy response
                        let res_msg = eraftpb::Message::default();
                        let res = InstallSnapshotResponse {
                            message: Some(res_msg),
                        };
                        Ok(Response::new(res))
                    }
                    Err(e) => {
                        tracing::error!("Failed to step raft node: {:?}", e);
                        return Err(Status::internal("Raft node step failed"));
                    }
                }
            }
            None => {
                tracing::error!("InstallSnapshot message is missing");
                return Err(Status::invalid_argument("InstallSnapshot message is missing"));
            }
        }

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
