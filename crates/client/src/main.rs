mod api;
mod config;
mod randomiser;
mod trade;
mod worker;

use config::Config;
use eyre::Result;
use maiya::{Resource, logs::Logger};
use uuid::Uuid;
use worker::Worker;

#[tokio::main]
async fn main() -> Result<()> {
    let client_id = Uuid::new_v4();
    let resource = Resource::builder().with_service_name("client").build();
    let logger = Logger::new(&resource, "client")?;

    let config_path = std::env::var("CONFIG_PATH")?;
    let market = std::env::var("MARKET_FEED_URL")?;
    let orderbook = std::env::var("ORDERBOOK_URL")?;
    let config = Config::new(config_path)?;

    let worker = Worker::new(client_id, market, orderbook, config)?;
    let result = worker.run().await;

    logger.shutdown()?;

    result
}
