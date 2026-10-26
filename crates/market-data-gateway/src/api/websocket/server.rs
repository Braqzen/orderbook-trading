//! Accepts client WebSocket connections.
//!
//! Each accepted socket runs as its own connection task.
//! That task receives prices only for instruments the client currently selected.

use crate::{
    api::{price::PriceChannels, websocket::connection::Connection},
    metrics::GatewayMetrics,
    whitelist::InstrumentWhitelist,
};
use eyre::Result;
use std::net::SocketAddr;
use tokio::{
    net::TcpListener,
    select,
    task::{JoinError, JoinSet},
};
use tokio_util::sync::CancellationToken;
use tracing::error;

pub struct WsServer {
    /// Socket to run the server on
    socket: SocketAddr,
    /// Instrument-specific price channels
    prices: PriceChannels,
    /// Instruments clients may subscribe to
    instruments: InstrumentWhitelist,
    /// Tracks metrics
    metrics: GatewayMetrics,
}

impl WsServer {
    pub fn new(
        socket: SocketAddr,
        prices: PriceChannels,
        instruments: InstrumentWhitelist,
        metrics: GatewayMetrics,
    ) -> Self {
        Self {
            socket,
            prices,
            instruments,
            metrics,
        }
    }

    pub async fn run(self, token: CancellationToken) -> Result<()> {
        let listener = TcpListener::bind(self.socket).await?;
        let mut connections = JoinSet::new();

        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                result = connections.join_next(), if !connections.is_empty() => {
                    if let Some(result) = result {
                        log_connection_result(result);
                    }
                }

                accepted = listener.accept() => {
                    let (stream, client) = match accepted {
                        Ok(value) => value,
                        Err(error) => {
                            error!(%error, "Failed to accept WebSocket connection");
                            continue;
                        }
                    };

                    connections.spawn(
                        Connection::new(
                            stream,
                            client,
                            self.prices.clone(),
                            self.instruments.clone(),
                            token.child_token(),
                            self.metrics.clone(),
                        )
                        .run(),
                    );
                }
            }
        }

        token.cancel();

        while let Some(result) = connections.join_next().await {
            log_connection_result(result);
        }

        Ok(())
    }
}

fn log_connection_result(result: std::result::Result<Result<()>, JoinError>) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => error!(%error, "WebSocket connection failed"),
        Err(error) => error!(%error, "WebSocket connection task failed"),
    }
}
