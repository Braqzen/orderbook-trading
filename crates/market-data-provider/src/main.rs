mod api;
mod metrics;
mod worker;

use eyre::{Result, eyre};
use maiya::{Resource, logs::Logger, metrics::Metrics};
use std::{net::SocketAddr, str::FromStr};
use worker::Worker;

// Include the generated proto types
pub mod proto {
    tonic::include_proto!("marketdataprovider");
}

#[tokio::main]
async fn main() -> Result<()> {
    let resource = Resource::builder()
        .with_service_name("market-data-provider")
        .build();
    let logger = Logger::new(&resource, "market-data-provider")?;
    let metrics = Metrics::new(&resource)?;

    let ws = std::env::var("WS")?;
    let socket = std::env::var("SOCKET")?;

    let ws = SocketAddr::from_str(&ws)?;
    let socket = SocketAddr::from_str(&socket)?;
    let worker = Worker::new(socket, ws);

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
