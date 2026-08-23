mod api;
mod grpc;
mod metrics;
mod state;
mod worker;

use eyre::Result;
use maiya::{Resource, logs::Logger, metrics::Metrics};
use std::{net::SocketAddr, str::FromStr};
use worker::Worker;

pub mod proto {
    tonic::include_proto!("generatorfeed");
}

#[tokio::main]
async fn main() -> Result<()> {
    let resource = Resource::builder().with_service_name("market-feed").build();
    let logger = Logger::new(&resource, "market-feed")?;
    let metrics = Metrics::new(&resource)?;

    let ws = std::env::var("WS")?;
    let socket = std::env::var("SOCKET")?;

    let ws = SocketAddr::from_str(&ws)?;
    let socket = SocketAddr::from_str(&socket)?;

    let worker = Worker::new(socket, ws);
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
