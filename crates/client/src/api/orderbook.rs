use crate::{api::Response, trade::Order};
use eyre::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::{
    select,
    sync::mpsc::{Receiver, Sender, error::TrySendError},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub struct OrderBook {
    url: String,
    order_receiver_channel: Receiver<Order>,
    response_sender_channel: Sender<Response>,
}

impl OrderBook {
    pub fn new(
        url: String,
        order_receiver_channel: Receiver<Order>,
        response_sender_channel: Sender<Response>,
    ) -> Self {
        Self {
            url,
            order_receiver_channel,
            response_sender_channel,
        }
    }

    pub async fn run(mut self, token: CancellationToken) -> Result<()> {
        let (mut stream, _response) = connect_async(self.url.clone()).await?;

        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                message = stream.next() => {
                    match message {
                        Some(Ok(Message::Text(payload))) => {
                            let response = match serde_json::from_str::<Response>(&payload) {
                                Ok(response) => response,
                                Err(error) => {
                                    warn!(%error, "Received invalid response");
                                    continue;
                                }
                            };

                            match self.response_sender_channel.try_send(response) {
                                Ok(()) => {}
                                Err(TrySendError::Closed(_)) => {
                                    error!("Engine response channel closed");
                                    break;
                                }
                                Err(TrySendError::Full(_)) => {
                                    warn!("Engine response queue full");
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

                order = self.order_receiver_channel.recv() => {
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
            }
        }

        if let Err(error) = stream.close(None).await {
            error!(%error, "Failed to close orderbook connection");
        }

        Ok(())
    }
}
