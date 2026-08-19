mod engine;
mod trade;
mod websocket;
mod worker;

use eyre::Result;
use maiya::{Resource, logs::Logger};
use std::{net::SocketAddr, str::FromStr};
use worker::Worker;

#[tokio::main]
async fn main() -> Result<()> {
    let resource = Resource::builder().with_service_name("orderbook").build();
    let logger = Logger::new(&resource, "orderbook")?;

    let ws = std::env::var("WS")?;
    let instrument = std::env::var("INSTRUMENT")?;
    let ws = SocketAddr::from_str(&ws)?;

    let worker = Worker::new(ws, instrument)?;
    let result = worker.run().await;

    logger.shutdown()?;

    result
}
