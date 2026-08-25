mod api;
mod engine;
mod metrics;
mod trade;
mod worker;

use eyre::Result;
use maiya::{Resource, logs::Logger, metrics::Metrics};
use std::{net::SocketAddr, str::FromStr};
use worker::Worker;

#[tokio::main]
async fn main() -> Result<()> {
    let resource = Resource::builder().with_service_name("orderbook").build();
    let logger = Logger::new(&resource, "orderbook")?;
    let metrics = Metrics::new(&resource)?;

    let ws = std::env::var("WS")?;
    let instrument = std::env::var("INSTRUMENT")?;
    let ws = SocketAddr::from_str(&ws)?;

    let worker = Worker::new(ws, instrument)?;
    let result = worker.run().await;

    let logger_shutdown = logger.shutdown();
    let metrics_shutdown = metrics.shutdown();

    if let Err(error) = result {
        return Err(error);
    }

    if let Err(error) = logger_shutdown {
        return Err(error.into());
    }

    if let Err(error) = metrics_shutdown {
        return Err(error.into());
    }

    Ok(())
}
