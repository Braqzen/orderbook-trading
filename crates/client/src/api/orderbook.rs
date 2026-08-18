use crate::trade::Order;
use eyre::Result;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::{select, sync::mpsc::Receiver};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

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
                        Some(Ok(Message::Text(payload))) => {
                            let response = match serde_json::from_str::<OrderResponse>(&payload) {
                                Ok(response) => response,
                                Err(error) => {
                                    warn!(%error, "Received invalid order response");
                                    continue;
                                }
                            };

                            match response {
                                OrderResponse::Rejected { order_id, reason } => {
                                    warn!(order = %order_id, %reason, "Order rejected");
                                }
                                OrderResponse::Unfilled { order_id } => {
                                    info!(order = %order_id, "Order accepted without fills");
                                }
                                OrderResponse::PartiallyFilled {
                                    order_id,
                                    filled_size,
                                    remaining_size,
                                } => {
                                    info!(
                                        order = %order_id,
                                        filled_size,
                                        remaining_size,
                                        "Order partially filled"
                                    );
                                }
                                OrderResponse::Filled {
                                    order_id,
                                    filled_size,
                                } => {
                                    info!(order = %order_id, filled_size, "Order filled");
                                }
                            }
                        }
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

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum OrderResponse {
    Rejected {
        order_id: Uuid,
        reason: String,
    },
    Unfilled {
        order_id: Uuid,
    },
    PartiallyFilled {
        order_id: Uuid,
        filled_size: u64,
        remaining_size: u64,
    },
    Filled {
        order_id: Uuid,
        filled_size: u64,
    },
}
