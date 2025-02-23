fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_files = &[
        "src/core/raft/proto/raft.proto",
        "src/core/raft/proto/eraftpb.proto"
    ];

    tonic_build::configure()
        .build_server(true)
        .compile_protos(proto_files, &["src/core/raft/proto"])?;

    Ok(())
}
