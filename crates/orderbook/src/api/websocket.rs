use crate::{
    api::{ConnectionRegistry, connection::Connection},
    metrics::OrderbookMetrics,
    trade::{Instrument, Request},
};
use eyre::Result;
use std::net::SocketAddr;
use tokio::{
    net::TcpListener,
    select,
    sync::mpsc::Sender,
    task::{JoinError, JoinSet},
};
use tokio_util::sync::CancellationToken;
use tracing::error;

pub struct WsServer {
    /// Bind the server to this socket
    socket: SocketAddr,
    instrument: Instrument,
    /// Send order requests from a client to the trading engine
    order_sender_channel: Sender<Request>,
    /// Track which clients are currently connected so we can stream their trades to them
    connection_registry: ConnectionRegistry,
    metrics: OrderbookMetrics,
}

impl WsServer {
    pub fn new(
        socket: SocketAddr,
        instrument: Instrument,
        order_sender_channel: Sender<Request>,
        connection_registry: ConnectionRegistry,
        metrics: OrderbookMetrics,
    ) -> Self {
        Self {
            socket,
            instrument,
            order_sender_channel,
            connection_registry,
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
                            self.order_sender_channel.clone(),
                            token.child_token(),
                            self.connection_registry.clone(),
                            self.metrics.clone(),
                        )
                        .run(),
                    );
                }

                // Disconnected clients are (error) logged
                connection_ended = connections.join_next(), if !connections.is_empty() => {
                    if let Some(result) = connection_ended {
                        log_connection_result(&self.instrument, result);
                    }
                }
            }
        }

        token.cancel();

        while let Some(result) = connections.join_next().await {
            log_connection_result(&self.instrument, result);
        }

        Ok(())
    }
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
