use crate::proto::PriceUpdate;
use eyre::Result;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::{
    net::TcpListener,
    select,
    sync::broadcast::{Sender, error::RecvError},
    task::{JoinError, JoinSet},
};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub struct WsServer {
    socket: SocketAddr,
    price_channel: Sender<PriceUpdate>,
}

impl WsServer {
    pub fn new(socket: SocketAddr, price_channel: Sender<PriceUpdate>) -> Self {
        Self {
            socket,
            price_channel,
        }
    }

    pub async fn run(&self, token: CancellationToken) -> Result<()> {
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
                    let (stream, peer) = match accepted {
                        Ok(value) => value,
                        Err(error) => {
                            error!(%error, "Failed to accept WebSocket connection");
                            continue;
                        }
                    };

                    let price_channel = self.price_channel.clone();
                    let connection_token = token.child_token();

                    connections.spawn(async move {
                        let mut ws = match accept_async(stream).await {
                            Ok(ws) => ws,
                            Err(error) => {
                                error!(%error, "WebSocket handshake failed");
                                return Ok(());
                            }
                        };

                        let mut price_receiver = price_channel.subscribe();

                        loop {
                            select! {
                                biased;

                                _ = connection_token.cancelled() => break,

                                message = ws.next() => {
                                    match message {
                                        Some(Ok(Message::Close(_))) | None => break,
                                        Some(Err(error)) => {
                                            error!(%error, "WebSocket connection failed");
                                            break;
                                        }
                                        _ => {}
                                    }
                                }

                                price_update = price_receiver.recv() => {
                                    match price_update {
                                        Ok(price_update) => {
                                            info!(
                                                %peer,
                                                instrument = price_update.instrument,
                                                price = price_update.value,
                                                "Sending price"
                                            );

                                            let payload = serde_json::json!({
                                                "instrument": price_update.instrument,
                                                "value": price_update.value,
                                            });

                                            if let Err(error) = ws
                                                .send(Message::Text(payload.to_string().into()))
                                                .await
                                            {
                                                error!(%error, "Failed to send price");
                                                break;
                                            }
                                        }
                                        Err(RecvError::Lagged(skipped)) => {
                                            warn!(skipped, "WebSocket client lagged");
                                        }
                                        Err(RecvError::Closed) => break,
                                    }
                                }
                            }
                        }

                        if let Err(error) = ws.close(None).await {
                            error!(%error, "Failed to close WebSocket connection");
                        }

                        Ok(())
                    });
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
