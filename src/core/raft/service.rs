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
                    let mut state = self.state_machine.lock().await;

                    match op {
                        Operation::Increment => {
                            tracing::debug!("Incrementing, Current state: {:?}", 0);
                            state.increment();
                        },
                        Operation::Decrement => {
                            tracing::debug!("Decrementing, Current state: {:?}", 0);
                            state.decrement();
                        },
                        Operation::IncrementBy(val) => {
                            tracing::debug!("IncrementingBy: {:?}, Current state: {:?}", val, 0);
                            state.increment_by(val);
                        },
                        Operation::DecrementBy(val) => {
                            tracing::debug!("DecrementingBy: {:?}, Current state: {:?}", val, 0);
                            state.decrement_by(val);
                        },
                        Operation::Set(val) => {
                            tracing::debug!("Set: {:?}, Current state: {:?}", val, 0);
                            state.set(val)
                        },
                    }
                }
                Err(e) => {
                    tracing::error!("Error decoding entry data: {:?}", e);
                }
            }
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
