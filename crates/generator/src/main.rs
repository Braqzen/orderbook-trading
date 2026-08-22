mod config;
mod feed;
mod instrument;
mod price;
mod publisher;
mod worker;

use crate::{config::Config, worker::Worker};
use eyre::Result;
use maiya::{Resource, logs::Logger};

pub mod proto {
    tonic::include_proto!("generatorfeed");
}

#[tokio::main]
async fn main() -> Result<()> {
    let resource = Resource::builder().with_service_name("generator").build();
    let logger = Logger::new(&resource, "generator")?;

    let market_feed_url = std::env::var("MARKET_FEED_URL")?;
    let config_path = std::env::var("CONFIG_PATH")?;
    let config = Config::new(config_path)?;

    let worker = Worker::new(market_feed_url, config)?;
    let result = worker.run().await;

    logger.shutdown()?;

    result
}
