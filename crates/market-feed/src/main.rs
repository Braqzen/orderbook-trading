mod api;
mod worker;

use eyre::{Result, eyre};
use maiya::logs::Logger;
use opentelemetry_sdk::Resource;
use std::{net::SocketAddr, str::FromStr};
use worker::Worker;

pub mod proto {
    tonic::include_proto!("generatorfeed");
}

#[tokio::main]
async fn main() -> Result<()> {
    let resource = Resource::builder().with_service_name("market-feed").build();
    let logger = Logger::new(&resource, "market-feed").map_err(|error| eyre!("{error}"))?;

    let ws = std::env::var("WS")?;
    let socket = std::env::var("SOCKET")?;

    let ws = SocketAddr::from_str(&ws)?;
    let socket = SocketAddr::from_str(&socket)?;

    let mut worker = Worker::new(socket, ws);
    let result = worker.run().await;

    logger.shutdown().map_err(|error| eyre!("{error}"))?;

    result
}
