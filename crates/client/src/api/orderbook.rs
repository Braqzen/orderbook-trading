use crate::trade::Order;
use eyre::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::{select, sync::mpsc::Receiver};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub struct OrderBook {
    url: String,
}

impl OrderBook {
    pub fn new(url: String) -> Self {
        Self { url }
    }

    pub async fn run(&self, mut receiver: Receiver<Order>, token: CancellationToken) -> Result<()> {
        let (mut stream, _response) = connect_async(self.url.clone()).await?;

        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                order = receiver.recv() => {
                    let Some(order) = order else {
                        error!("Engine to orderbook api channel closed");
                        break;
                    };

                    let payload = match serde_json::to_string(&order) {
                        Ok(payload) => payload,
                        Err(error) => {
                            error!(%error, "Failed to serialize order");
                            continue;
                        }
                    };

                    match stream.send(Message::Text(payload.into())).await {
                        Ok(()) => {
                            info!(price = order.price, size = order.size, side = %order.side, "Order sent to orderbook");
                        },
                        Err(error) => {
                            error!(%error, "Failed to send order to orderbook");
                            break;
                        }
                    }
                }

                message = stream.next() => {
                    match message {
                        Some(Ok(Message::Close(_))) => {
                            error!("Orderbook service explicitly closed connection");
                            break;
                        }
                        Some(Err(error)) => {
                            error!(%error, "Unknown error");
                            break;
                        }
                        None => {
                            error!("Disconnected from orderbook service");
                            break;
                        }
                        _ => {
                            warn!("Skipping unexpected message");
                        }
                    }
                }
            }
        }

        if let Err(error) = stream.close(None).await {
            error!(%error, "Failed to close orderbook connection");
        }

        Ok(())
    }
}
