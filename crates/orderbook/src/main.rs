mod api;
mod engine;
mod metrics;
mod trade;
mod worker;

use eyre::{Result, eyre};
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

    // Report all errors
    let shutdown_errors = [
        logger_shutdown.err().map(|e| format!("logger: {e}")),
        metrics_shutdown.err().map(|e| format!("metrics: {e}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("; ");

    match (result, shutdown_errors.is_empty()) {
        (Ok(()), true) => Ok(()),
        (Err(error), true) => Err(error),
        (Ok(()), false) => Err(eyre!(shutdown_errors)),
        (Err(error), false) => Err(error.wrap_err(shutdown_errors)),
    }
}
