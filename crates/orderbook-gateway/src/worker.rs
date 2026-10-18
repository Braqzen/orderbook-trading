//! The worker runs the orderbook gateway for one instrument.
//!
//! It accepts client WebSocket connections, forwards their orders to the orderbook,
//! and streams each client's executions from Redpanda until shutdown.

use crate::{api::WsServer, environment::Environment, submit::Submitter, trade::Instrument};
use eyre::Result;
use std::{net::SocketAddr, str::FromStr};
use tokio::{
    select,
    signal::unix::{SignalKind, signal},
    task::JoinError,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub struct WorkerConfig {
    /// Bind to this socket to accept trading client WebSocket connections
    ws: SocketAddr,
    /// The only instrument this gateway accepts
    instrument: Instrument,
    /// gRPC address of the orderbook for this instrument
    orderbook_url: String,
    /// Redpanda brokers the gateway reads executions from
    redpanda_brokers: String,
}

impl WorkerConfig {
    pub fn new(environment: Environment) -> Result<Self> {
        Ok(Self {
            ws: SocketAddr::from_str(&environment.ws)?,
            instrument: Instrument::try_from(environment.instrument.as_str())?,
            orderbook_url: environment.orderbook_url,
            redpanda_brokers: environment.redpanda_brokers,
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

        let submitter = Submitter::connect(&self.config.orderbook_url).await?;
        let server = WsServer::new(
            self.config.ws,
            self.config.instrument.clone(),
            submitter,
            self.config.redpanda_brokers,
        );

        let token = CancellationToken::new();
        let mut server = tokio::spawn(server.run(token.child_token()));

        select! {
            result = &mut server => log_task_result(&self.config.instrument, result),
            _ = sigint.recv() => {
                info!(instrument = %self.config.instrument, "Received interrupt signal");
                token.cancel();
                log_task_result(&self.config.instrument, server.await);
            }
            _ = sigterm.recv() => {
                info!(instrument = %self.config.instrument, "Received terminate signal");
                token.cancel();
                log_task_result(&self.config.instrument, server.await);
            }
        }

        Ok(())
    }
}

fn log_task_result(instrument: &Instrument, result: std::result::Result<Result<()>, JoinError>) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => error!(%instrument, %error, "Service failed"),
        Err(error) => error!(%instrument, %error, "Service task failed"),
    }
}
