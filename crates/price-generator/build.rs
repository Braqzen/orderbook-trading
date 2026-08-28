/// Magic used to turn a proto file into Rust types

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(false)
        .compile_protos(
            &["../../proto/market-data-provider.proto"],
            &["../../proto"],
        )?;
    Ok(())
}
