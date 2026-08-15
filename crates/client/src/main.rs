mod feed;
mod orderbook;
mod worker;

use eyre::Result;
use maiya::telemetry::Telemetry;
use worker::Worker;

#[tokio::main]
async fn main() -> Result<()> {
    // let telemetry = Telemetry::init("client")?;

    let market = std::env::var("MARKET_FEED_URL")?;
    let orderbook = std::env::var("ORDERBOOK_URL")?;

    let worker = Worker::new(market, orderbook);
    let result = worker.run().await;

    // telemetry.shutdown()?;

    result
}
