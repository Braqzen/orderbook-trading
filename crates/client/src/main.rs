mod api;
mod config;
mod metrics;
mod randomiser;
mod trade;
mod worker;

use api::WsUrl;
use config::Config;
use eyre::{Result, eyre};
use maiya::{Resource, logs::Logger, metrics::Metrics};
use worker::Worker;

#[tokio::main]
async fn main() -> Result<()> {
    let resource = Resource::builder().with_service_name("client").build();
    let logger = Logger::new(&resource, "client")?;
    let metrics = Metrics::new(&resource)?;

    let config_path = std::env::var("CONFIG_PATH")?;
    let market_data_provider_url = std::env::var("MARKET_DATA_PROVIDER_URL")?;

    let config = Config::new(config_path)?;
    let market_data_provider_url = WsUrl::try_from(market_data_provider_url.as_str())?;
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
