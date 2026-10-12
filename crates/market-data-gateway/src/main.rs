mod api;
mod environment;
mod metrics;
mod worker;

use environment::Environment;
use eyre::{Result, eyre};
use maiya::{Resource, logs::Logger, metrics::Metrics};
use worker::{Worker, WorkerConfig};

#[tokio::main]
async fn main() -> Result<()> {
    let resource = Resource::builder()
        .with_service_name("market-data-gateway")
        .build();
    let logger = Logger::new(&resource, "market-data-gateway")?;
    let metrics = Metrics::new(&resource)?;

    let config = WorkerConfig::new(Environment::load()?)?;
    let worker = Worker::new(config);

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
