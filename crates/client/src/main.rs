mod api;
mod config;
mod metrics;
mod randomiser;
mod trade;
mod worker;

use config::Config;
use eyre::Result;
use maiya::{Resource, logs::Logger, metrics::Metrics};
use uuid::Uuid;
use worker::Worker;

#[tokio::main]
async fn main() -> Result<()> {
    let client_id = Uuid::new_v4();
    let resource = Resource::builder().with_service_name("client").build();
    let logger = Logger::new(&resource, "client")?;
    let metrics = Metrics::new(&resource)?;

    let config_path = std::env::var("CONFIG_PATH")?;
    let market_data_provider_url = std::env::var("MARKET_DATA_PROVIDER_URL")?;
    let config = Config::new(config_path)?;

    let worker = Worker::new(client_id, market_data_provider_url, config)?;
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
