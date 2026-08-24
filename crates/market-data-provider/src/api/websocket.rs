use crate::{api::connection::Connection, metrics::MarketDataProviderMetrics, proto::PriceUpdate};
use eyre::Result;
use std::net::SocketAddr;
use tokio::{
    net::TcpListener,
    select,
    sync::broadcast::Sender,
    task::{JoinError, JoinSet},
};
use tokio_util::sync::CancellationToken;
use tracing::error;

pub struct WsServer {
    socket: SocketAddr,
    price_sender_channel: Sender<PriceUpdate>,
    metrics: MarketDataProviderMetrics,
}

impl WsServer {
    pub fn new(socket: SocketAddr, price_sender_channel: Sender<PriceUpdate>) -> Self {
        Self {
            socket,
            price_sender_channel,
            metrics: MarketDataProviderMetrics::new(),
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

                    let connection_token = token.child_token();
                    let price_receiver_channel = self.price_sender_channel.subscribe();

                    connections.spawn(
                        Connection::new(
                            stream,
                            client,
                            price_receiver_channel,
                            connection_token,
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
