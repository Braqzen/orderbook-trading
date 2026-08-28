mod config;
mod instrument;
mod metrics;
mod publisher;
mod simulation;
mod worker;

use crate::{config::Config, worker::Worker};
use eyre::{Result, eyre};
use maiya::{Resource, logs::Logger, metrics::Metrics};

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

    let worker_err = result.err();
    let logger_err = logger_shutdown.err();
    let metrics_err = metrics_shutdown.err();

    match (worker_err, logger_err, metrics_err) {
        (None, None, None) => Ok(()),
        (Some(e), None, None) => Err(e),
        (None, Some(e), None) => Err(e.into()),
        (None, None, Some(e)) => Err(e.into()),
        (Some(w), Some(l), Some(m)) => Err(w.wrap_err(format!("logger: {l}; metrics: {m}"))),
        (Some(w), Some(l), None) => Err(w.wrap_err(format!("logger: {l}"))),
        (Some(w), None, Some(m)) => Err(w.wrap_err(format!("metrics: {m}"))),
        (None, Some(l), Some(m)) => Err(eyre!("logger: {l}; metrics: {m}")),
    }
}
