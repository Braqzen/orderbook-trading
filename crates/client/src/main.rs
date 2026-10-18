mod api;
mod config;
mod environment;
mod metrics;
mod randomiser;
mod trade;
mod valkey;
mod worker;

use api::WsUrl;
use config::Config;
use environment::Environment;
use eyre::{Result, eyre};
use maiya::{Resource, logs::Logger, metrics::Metrics};
use valkey::SequenceCache;
use worker::Worker;

#[tokio::main]
async fn main() -> Result<()> {
    let resource = Resource::builder().with_service_name("client").build();
    let logger = Logger::new(&resource, "client")?;
    let metrics = Metrics::new(&resource)?;

    // TODO: create Environment and Configs similar to other crates
    let environment = Environment::load()?;
    let config = Config::new(environment.config_path)?;
    let market_data_gateway_url = WsUrl::try_from(environment.market_data_gateway_url.as_str())?;
    let sequences = SequenceCache::connect(&environment.valkey_url).await?;
    let worker = Worker::new(market_data_gateway_url, config, sequences)?;

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
