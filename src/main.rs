mod core;
mod tests;

use core::raft::service::RaftService;
use core::raft::node::{new_config, start_raft};
use core::raft::rpc::proto::raft_server::RaftServer;

use tonic::transport::Server;
use std::net::SocketAddr;

use core::counter::Counter;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter };
use tracing_bunyan_formatter::{JsonStorageLayer, BunyanFormattingLayer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app_name = concat!(env!("CARGO_PKG_NAME"), "-", env!("CARGO_PKG_VERSION")).to_string();

    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    let file_appender = tracing_appender::rolling::daily("./logs", "trace.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let bunyan_formatting_layer = BunyanFormattingLayer::new(app_name, non_blocking);
    //let fmt_layer = fmt::layer().with_writer(move || non_blocking.clone());

    if let Err(e) = tracing_subscriber::registry()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(bunyan_formatting_layer) // Remove this line if you don't need file output
        .try_init() {
            eprintln!("Error initializing tracing: {}", e);
            panic!("Failed to initialise tracing")
    }



    let addr: SocketAddr = "[::1]:56789".parse().expect("Could not parse network address and port");
    let config = new_config(1).await;
    let (node, tx) = start_raft(&config).await;
    //let node = node.lock().await;
    let raft_service = RaftService::new(node);
    let raft_server = RaftServer::new(raft_service);

    let _ = Server::builder()
        .add_service(raft_server)
        .serve(addr)
        .await;


    let mut counter = Counter::new(0);
    counter.increment();
    counter.increment();

    Ok(())
}

