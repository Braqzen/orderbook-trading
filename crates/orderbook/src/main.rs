mod api;
mod engine;
mod environment;
mod execution;
mod metrics;
mod trade;
mod worker;

use environment::Environment;
use eyre::{Result, eyre};
use maiya::{Resource, logs::Logger, metrics::Metrics};
use worker::Worker;

pub mod proto {
    tonic::include_proto!("orderbook");
}

#[tokio::main]
async fn main() -> Result<()> {
    let resource = Resource::builder().with_service_name("orderbook").build();
    let logger = Logger::new(&resource, "orderbook")?;
    let metrics = Metrics::new(&resource)?;

    let worker = Worker::new(Environment::load()?)?;

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
