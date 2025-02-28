#!/bin/bash

#cargo build

# Start node 1
RUST_LOG=DEBUG cargo run -- --id 1 --bind-address 127.0.0.1 --port 50051 &

# Start node 2
RUST_LOG=DEBUG cargo run -- --id 2 --bind-address 127.0.0.1 --port 50052 &

# Start node 3
RUST_LOG=DEBUG cargo run -- --id 3 --bind-address 127.0.0.1 --port 50053 &

# Wait for all processes to finish (optional)
wait
