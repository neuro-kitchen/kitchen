//! Generates the wire types from `contract/proto/kitchen/runtime/v1/runtime.proto`.

const PROTO_DIR: &str = "../../contract/proto";
const PROTO: &str = "../../contract/proto/kitchen/runtime/v1/runtime.proto";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed={PROTO}");
    let files = protox::compile([PROTO], [PROTO_DIR])?;
    // The descriptor set too: the breaking-change test reads it
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
    std::fs::write(out.join("runtime.fds"), prost::Message::encode_to_vec(&files))?;
    prost_build::Config::new().compile_fds(files)?;
    Ok(())
}
