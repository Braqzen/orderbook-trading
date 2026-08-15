mod engine;
mod trade;
mod websocket;
mod worker;

use eyre::{Result, eyre};
use maiya::logs::Logger;
use opentelemetry_sdk::Resource;
use std::{net::SocketAddr, str::FromStr};
use worker::Worker;

#[tokio::main]
async fn main() -> Result<()> {
    let resource = Resource::builder().with_service_name("orderbook").build();
    let logger = Logger::new(&resource, "orderbook").map_err(|error| eyre!("{error}"))?;

    let ws = std::env::var("WS")?;
    let ws = SocketAddr::from_str(&ws)?;

    let worker = Worker::new(ws);
    let result = worker.run().await;

    logger.shutdown().map_err(|error| eyre!("{error}"))?;

    result
}
