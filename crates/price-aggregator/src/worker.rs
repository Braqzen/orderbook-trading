//! The worker runs the price aggregator.
//!
//! It receives price updates over gRPC and stores the latest price for each instrument in Valkey until shutdown.

use crate::{api::GrpcClient, environment::Environment, valkey::Cache};
use eyre::{Result, eyre};
use tokio::{
    select,
    signal::unix::{SignalKind, signal},
    task::JoinError,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub struct WorkerConfig {
    price_generator_url: String,
    valkey_url: String,
    ttl_seconds: u64,
}

impl WorkerConfig {
    pub fn new(environment: Environment) -> Result<Self> {
        let ttl_seconds = environment
            .ttl_seconds
            .parse::<u64>()
            .map_err(|error| eyre!("PRICE_TTL_SECONDS is invalid: {error}"))?;

        Ok(Self {
            price_generator_url: environment.price_generator_url,
            valkey_url: environment.valkey_url,
            ttl_seconds,
        })
    }
}

pub struct Worker {
    config: WorkerConfig,
}

impl Worker {
    pub fn new(config: WorkerConfig) -> Self {
        Self { config }
    }

    pub async fn run(self) -> Result<()> {
        // Handle running locally and interrupting the process with ctrl+c.
        let mut sigint = signal(SignalKind::interrupt())?;
        // Handle running in a container and terminating the process with docker stop.
        let mut sigterm = signal(SignalKind::terminate())?;

        let prices = Cache::connect(&self.config.valkey_url, self.config.ttl_seconds).await?;
        let grpc = GrpcClient::new(self.config.price_generator_url, prices);

        let token = CancellationToken::new();
        let mut client = tokio::spawn(grpc.run(token.clone()));

        select! {
            result = &mut client => log_client_result(result),
            _ = sigint.recv() => {
                info!("Received interrupt signal");
                token.cancel();
                log_client_result(client.await);
            }
            _ = sigterm.recv() => {
                info!("Received terminate signal");
                token.cancel();
                log_client_result(client.await);
            }
        };

        Ok(())
    }
}

fn log_client_result(result: std::result::Result<Result<()>, JoinError>) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => error!(%error, "Service failed"),
        Err(error) => error!(%error, "Service task failed"),
    }
}
