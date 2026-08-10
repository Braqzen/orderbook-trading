mod price;
mod worker;

use crate::worker::Worker;
use eyre::Result;
use maiya::telemetry::Telemetry;

pub mod proto {
    tonic::include_proto!("generatorfeed");
}

#[tokio::main]
async fn main() -> Result<()> {
    // let telemetry = Telemetry::init("generator")?;
    let api = std::env::var("MARKET_FEED_URL")?;
    let start_price = std::env::var("START_PRICE")?.parse()?;
    let upper_limit = std::env::var("UPPER_LIMIT")?.parse()?;
    let lower_limit = std::env::var("LOWER_LIMIT")?.parse()?;

    let mut worker = Worker::new(api, start_price, upper_limit, lower_limit);
    let result = worker.run().await;

    // telemetry.shutdown()?;

    result
}
