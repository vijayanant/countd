mod core;
mod tests;

use clap::{Parser, command, arg};

use core::raft::service::RaftService;
use core::raft::node::create_raft_service;
use core::raft::rpc::proto::raft_server::RaftServer;

use tonic::transport::Server;
use tokio::sync::Mutex;
use std::net::SocketAddr;
use std::collections::HashMap;
use std::sync::Arc;
use core::counter::Counter;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter };
use tracing_bunyan_formatter::{JsonStorageLayer, BunyanFormattingLayer};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 0)]
    id: u64,
    #[arg(short, long, default_value_t = String::from("127.0.0.1:56789"))]
    address: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_command_line_args();

    init_tracing();

    let raft_service = create_raft_service(args.id);;

    run_grpc_server(args.address, raft_service).await?;

    Ok(())
}

fn parse_command_line_args() -> Args {
    Args::parse()
}

fn init_tracing() {
    let app_name = concat!(env!("CARGO_PKG_NAME"), "-", env!("CARGO_PKG_VERSION")).to_string();
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    let file_appender = tracing_appender::rolling::daily("./logs", "trace.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let bunyan_formatting_layer = BunyanFormattingLayer::new(app_name, non_blocking);

    if let Err(e) = tracing_subscriber::registry()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(bunyan_formatting_layer) // Remove this line if you don't need file output
        .try_init() {
            eprintln!("Error initializing tracing: {}", e);
            panic!("Failed to initialise tracing")
    }
}



async fn run_grpc_server(address: String, raft_service: RaftService)->Result<(), Box<dyn std::error::Error>> {

    let addr: SocketAddr = address.parse()?;
    println!("Server listening on {:?}", addr);

    let raft_server = RaftServer::new(raft_service);

    let _ = Server::builder()
        .add_service(raft_server)
        .serve(addr)
        .await?;

    Ok(())
}

