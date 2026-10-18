//! WebSocket entry point for one instrument.
//! It listens for clients and spawns a connection handler for each accepted socket.

use crate::{
    api::{SessionStore, connection::Connection},
    execution::ExecutionReader,
    metrics::GatewayMetrics,
    submit::Submitter,
    trade::Instrument,
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
    /// Bind the server to this socket
    socket: SocketAddr,
    /// Used for logging
    instrument: Instrument,
    /// Send order requests from a client to the orderbook
    submitter: Submitter,
    /// Redpanda brokers executions are read from
    brokers: String,
    /// Track which clients are currently connected so we can stream their trades to them
    sessions: SessionStore,
    metrics: GatewayMetrics,
}

impl WsServer {
    pub fn new(
        socket: SocketAddr,
        instrument: Instrument,
        submitter: Submitter,
        brokers: String,
    ) -> Self {
        let metrics = GatewayMetrics::new(&instrument);
        let sessions = SessionStore::new();

        Self {
            socket,
            instrument,
            submitter,
            brokers,
            sessions,
            metrics,
        }
    }

    pub async fn run(self, token: CancellationToken) -> Result<()> {
        let listener = TcpListener::bind(self.socket).await?;
        let mut connections = JoinSet::new();

        loop {
            select! {
                _ = token.cancelled() => break,

                // New connections are spawned in their own tokio tasks
                connection = listener.accept() => {
                    let (stream, _) = match connection {
                        Ok(value) => value,
                        Err(error) => {
                            error!(instrument = %self.instrument, %error, "Failed to accept WebSocket connection");
                            continue;
                        }
                    };

                    connections.spawn(
                        Connection::new(
                            stream,
                            self.instrument.clone(),
                            self.submitter.clone(),
                            ExecutionReader::new(self.brokers.clone(), self.instrument.clone()),
                            token.child_token(),
                            self.sessions.clone(),
                            self.metrics.clone(),
                        )
                        .run(),
                    );
                }

                // Disconnected clients are (error) logged
                connection_ended = connections.join_next(), if !connections.is_empty() => {
                    if let Some(result) = connection_ended {
                        Self::log_connection_result(&self.instrument, result);
                    }
                }
            }
        }

        token.cancel();

        while let Some(result) = connections.join_next().await {
            Self::log_connection_result(&self.instrument, result);
        }

        Ok(())
    }

    fn log_connection_result(
        instrument: &Instrument,
        result: std::result::Result<Result<()>, JoinError>,
    ) {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => error!(%instrument, %error, "WebSocket connection failed"),
            Err(error) => error!(%instrument, %error, "WebSocket connection task failed"),
        }
    }
}
