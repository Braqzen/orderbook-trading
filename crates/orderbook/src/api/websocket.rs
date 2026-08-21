use crate::{
    api::{ConnectionRegistry, connection::Connection},
    trade::Request,
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
    /// Send order requests from a client to the trading engine
    order_sender_channel: Sender<Request>,
    /// Track which clients are currently connected so we can stream their trades to them
    connection_registry: ConnectionRegistry,
}

impl WsServer {
    pub fn new(
        socket: SocketAddr,
        order_sender_channel: Sender<Request>,
        connection_registry: ConnectionRegistry,
    ) -> Self {
        Self {
            socket,
            order_sender_channel,
            connection_registry,
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
                            error!(%error, "Failed to accept WebSocket connection");
                            continue;
                        }
                    };

                    connections.spawn(
                        Connection::new(
                            stream,
                            self.order_sender_channel.clone(),
                            token.child_token(),
                            self.connection_registry.clone(),
                        )
                        .run(),
                    );
                }

                // Disconnected clients are (error) logged
                connection_ended = connections.join_next(), if !connections.is_empty() => {
                    if let Some(result) = connection_ended {
                        log_connection_result(result);
                    }
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
