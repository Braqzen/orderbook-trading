//! The worker runs the market data gateway.
//!
//! It accepts client WebSocket connections and sends each client Valkey price updates
//! for the instruments that client is subscribed to until shutdown.
//!
//! Price updates are live (client must be connected to receive current value) and
//! there will not be a mechanism for replaying old prices for a trading client.

use crate::{
    api::{PriceChannels, PriceReader, WsServer},
    environment::Environment,
    metrics::GatewayMetrics,
    whitelist::InstrumentWhitelist,
};
use eyre::Result;
use std::{net::SocketAddr, str::FromStr};
use tokio::{
    signal::unix::{SignalKind, signal},
    task::{JoinError, JoinSet},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub struct WorkerConfig {
    /// Bind to this socket to accept trading client WebSocket connections
    ws: SocketAddr,
    /// Address of the Valkey service the gateway connects to
    valkey_url: String,
    /// Instruments clients may subscribe to
    instruments: InstrumentWhitelist,
}

impl WorkerConfig {
    pub fn new(environment: Environment) -> Result<Self> {
        let ws = SocketAddr::from_str(&environment.ws)?;
        let instruments = InstrumentWhitelist::new(&environment.instruments_config_path)?;

        Ok(Self {
            ws,
            valkey_url: environment.valkey_url,
            instruments,
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

        let metrics = GatewayMetrics::new();
        // Handles channels which are used to send instrument price updates from the cache to the client
        let prices = PriceChannels::new();

        // 2 background tasks, reading and forwarding price updates from cache and websocket server for client management
        let ws = WsServer::new(
            self.config.ws,
            prices.clone(),
            self.config.instruments,
            metrics,
        );
        let reader = PriceReader::new(self.config.valkey_url, prices);

        let token = CancellationToken::new();
        let ws_token = token.child_token();
        let valkey_token = token.child_token();

        let mut tasks = JoinSet::new();

        tasks.spawn(ws.run(ws_token));
        tasks.spawn(reader.run(valkey_token));

        tokio::select! {
            Some(result) = tasks.join_next() => log_task_result(result),
            _ = sigint.recv() => info!("Received interrupt signal"),
            _ = sigterm.recv() => info!("Received terminate signal"),
        }

        token.cancel();

        while let Some(result) = tasks.join_next().await {
            log_task_result(result);
        }

        Ok(())
    }
}

fn log_task_result(result: std::result::Result<Result<()>, JoinError>) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => error!(%error, "Service failed"),
        Err(error) => error!(%error, "Service task failed"),
    }
}
