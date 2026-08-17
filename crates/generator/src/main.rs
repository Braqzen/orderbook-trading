mod price;
mod worker;

use crate::worker::Worker;
use eyre::Result;
use maiya::{Resource, logs::Logger};

pub mod proto {
    tonic::include_proto!("generatorfeed");
}

#[tokio::main]
async fn main() -> Result<()> {
    let resource = Resource::builder().with_service_name("generator").build();
    let logger = Logger::new(&resource, "generator")?;

    let api = std::env::var("MARKET_FEED_URL")?;
    let start_price = std::env::var("START_PRICE")?.parse()?;
    let upper_limit = std::env::var("UPPER_LIMIT")?.parse()?;
    let lower_limit = std::env::var("LOWER_LIMIT")?.parse()?;

    let mut worker = Worker::new(api, start_price, upper_limit, lower_limit);
    let result = worker.run().await;

    logger.shutdown()?;

    result
}
