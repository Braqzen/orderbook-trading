mod config;
mod instrument;
mod metrics;
mod publisher;
mod simulation;
mod worker;

use crate::{config::Config, worker::Worker};
use eyre::{Result, eyre};
use maiya::{Resource, logs::Logger, metrics::Metrics};

// Include the generated proto types
pub mod proto {
    tonic::include_proto!("marketdataprovider");
}

#[tokio::main]
async fn main() -> Result<()> {
    let resource = Resource::builder()
        .with_service_name("price-generator")
        .build();
    let logger = Logger::new(&resource, "price-generator")?;
    let metrics = Metrics::new(&resource)?;

    let market_data_provider_url = std::env::var("MARKET_DATA_PROVIDER_URL")?;
    let config_path = std::env::var("CONFIG_PATH")?;

    let config = Config::new(config_path)?;
    let worker = Worker::new(market_data_provider_url, config)?;

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
