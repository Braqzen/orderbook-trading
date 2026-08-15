use crate::trade::{Order, OrderType, Price};
use eyre::Result;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::net::SocketAddr;
use tokio::{
    net::TcpListener,
    select,
    sync::mpsc::Sender,
    task::{JoinError, JoinSet},
};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};

pub struct WsServer;

impl WsServer {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(
        &self,
        tx: Sender<(Price, Order)>,
        ws: SocketAddr,
        token: CancellationToken,
    ) -> Result<()> {
        let listener = TcpListener::bind(ws).await?;
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
                    let (stream, _) = match accepted {
                        Ok(value) => value,
                        Err(error) => {
                            error!(%error, "Failed to accept WebSocket connection");
                            continue;
                        }
                    };

                    let tx = tx.clone();
                    let connection_token = token.child_token();

                    connections.spawn(async move {
                        let ws = match accept_async(stream).await {
                            Ok(ws) => ws,
                            Err(error) => {
                                error!(%error, "WebSocket handshake failed");
                                return Ok(());
                            }
                        };

                        let (mut sender, mut receiver) = ws.split();

                        loop {
                            select! {
                                biased;

                                _ = connection_token.cancelled() => break,

                                message = receiver.next() => {
                                    match message {
                                        Some(Ok(Message::Text(payload))) => {
                                            let order = match serde_json::from_str::<ApiOrder>(&payload) {
                                                Ok(order) => order,
                                                Err(error) => {
                                                    warn!(%error, "Received invalid order");
                                                    continue;
                                                }
                                            };

                                            let price = match Price::try_from(order.price) {
                                                Ok(price) => price,
                                                Err(error) => {
                                                    warn!(%error, "Received invalid order price");
                                                    continue;
                                                }
                                            };
                                            let order = Order::new(order.size, order.side);

                                            if tx.send((price, order)).await.is_err() {
                                                error!("Order channel closed");
                                                break;
                                            }
                                        }
                                        Some(Ok(Message::Close(_))) | None => break,
                                        Some(Err(error)) => {
                                            error!(%error, "WebSocket connection failed");
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }

                        if let Err(error) = sender.close().await {
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

#[derive(Deserialize)]
struct ApiOrder {
    price: f64,
    size: u64,
    side: OrderType,
}
