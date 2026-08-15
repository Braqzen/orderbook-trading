mod api;
mod trade;
mod worker;

use eyre::{Result, eyre};
use maiya::logs::Logger;
use opentelemetry_sdk::Resource;
use worker::Worker;

#[tokio::main]
async fn main() -> Result<()> {
    let resource = Resource::builder().with_service_name("client").build();
    let logger = Logger::new(&resource, "client").map_err(|error| eyre!("{error}"))?;

    let market = std::env::var("MARKET_FEED_URL")?;
    let orderbook = std::env::var("ORDERBOOK_URL")?;

    let worker = Worker::new(market, orderbook);
    let result = worker.run().await;

    logger.shutdown().map_err(|error| eyre!("{error}"))?;

    result
}
