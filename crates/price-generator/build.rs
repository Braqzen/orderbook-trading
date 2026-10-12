/// Magic used to turn a proto file into Rust types

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_client(false)
        .compile_protos(&["../../proto/price-generator.proto"], &["../../proto"])?;
    Ok(())
}
