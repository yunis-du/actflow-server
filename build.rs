fn main() -> Result<(), Box<dyn std::error::Error>> {
    built::write_built_file().expect("Failed to acquire build-time information");
    tonic_prost_build::configure()
        .build_client(false) // only build server code
        .compile_protos(&["proto/workflow.proto"], &["proto"])?;
    Ok(())
}
