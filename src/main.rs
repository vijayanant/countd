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
use std::io;
use core::counter::Counter;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Registry, EnvFilter, fmt};
use tracing_bunyan_formatter::{JsonStorageLayer, BunyanFormattingLayer};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 1)]
    id: u64,
    #[arg(short, long, default_value_t = String::from("127.0.0.1:56789"))]
    address: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let args = parse_command_line_args();
    let raft_service = create_raft_service(args.id)?;
    run_grpc_server(args.address, raft_service).await?;

    Ok(())
}

fn parse_command_line_args() -> Args {
    Args::parse()
}

fn init_tracing() {
    let app_name = concat!(env!("CARGO_PKG_NAME"), "-", env!("CARGO_PKG_VERSION")).to_string();
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("INFO"))
        .unwrap();

    let file_appender = tracing_appender::rolling::daily("./logs", "trace.log");
    let file_layer = fmt::Layer::new().with_writer(file_appender);

    //tracing_subscriber::registry()
    let subscriber = Registry::default()
        .with(env_filter)
        .with(file_layer);
    let _ = tracing::subscriber::set_global_default(subscriber).unwrap();


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

