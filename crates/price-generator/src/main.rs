mod config;
mod feed;
mod instrument;
mod metrics;
mod price;
mod publisher;
mod worker;

use crate::{config::Config, worker::Worker};
use eyre::Result;
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
