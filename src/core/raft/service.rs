use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use raft::{RawNode, eraftpb, storage::MemStorage};

use crate::core::raft::rpc::proto::{
    raft_server::Raft,
    RequestVoteRequest, RequestVoteResponse,
    AppendEntriesRequest, AppendEntriesResponse,
    InstallSnapshotRequest, InstallSnapshotResponse,
};


pub struct RaftService {
    node: Arc<Mutex<RawNode<MemStorage>>>,
}

impl RaftService {
    pub fn new(node: Arc<Mutex<RawNode<MemStorage>>>) -> Self {
        RaftService {node }
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
                let ready = {
                    let mut node = self.node.lock().await;
                    node.step(eraft_msg)
                };
                match ready {
                    Ok(ready) => {
                        //TODO process ready request
                        tracing::debug!("ready struct: {:?}", ready);
                        //Dummy response
                        let reply_msg = eraftpb::Message::default();
                        let res = RequestVoteResponse {
                            message: Some(reply_msg),
                        };
                        Ok(Response::new(res))
                    }
                    Err(e) =>{
                        tracing::error!("Failed to step raft node: {:?}", e);
                        return Err(Status::internal("Raft node step failed"));
                    }
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
