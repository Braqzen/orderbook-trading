mod api;
mod trade;
mod worker;

use eyre::Result;
use maiya::{Resource, logs::Logger};
use worker::Worker;

#[tokio::main]
async fn main() -> Result<()> {
    let resource = Resource::builder().with_service_name("client").build();
    let logger = Logger::new(&resource, "client")?;

    let inventory = std::env::var("INVENTORY")?;
    let market = std::env::var("MARKET_FEED_URL")?;
    let orderbook = std::env::var("ORDERBOOK_URL")?;

    let worker = Worker::new(market, orderbook, inventory)?;
    let result = worker.run().await;

    logger.shutdown()?;

    result
}
