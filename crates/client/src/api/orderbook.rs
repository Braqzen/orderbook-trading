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
use uuid::Uuid;

pub struct OrderBook {
    client_id: Uuid,
    url: String,
    order_receiver_channel: Receiver<Order>,
    response_sender_channel: Sender<Response>,
}

impl OrderBook {
    pub fn new(
        client_id: Uuid,
        url: String,
        order_receiver_channel: Receiver<Order>,
        response_sender_channel: Sender<Response>,
    ) -> Self {
        Self {
            client_id,
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
                                    warn!(client = %self.client_id, %error, "Received invalid response");
                                    continue;
                                }
                            };

                            match self.response_sender_channel.try_send(response) {
                                Ok(()) => {}
                                Err(TrySendError::Closed(_)) => {
                                    error!(client = %self.client_id, "Engine response channel closed");
                                    break;
                                }
                                Err(TrySendError::Full(_)) => {
                                    warn!(client = %self.client_id, "Engine response queue full");
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            error!(client = %self.client_id, "Orderbook service explicitly closed connection");
                            break;
                        }
                        Some(Err(error)) => {
                            error!(client = %self.client_id, %error, "Unknown error");
                            break;
                        }
                        None => {
                            error!(client = %self.client_id, "Disconnected from orderbook service");
                            break;
                        }
                        _ => {
                            warn!(client = %self.client_id, "Skipping unexpected message");
                        }
                    }
                }

                order = self.order_receiver_channel.recv() => {
                    let Some(order) = order else {
                        error!(client = %self.client_id, "Engine to orderbook api channel closed");
                        break;
                    };

                    let payload = match serde_json::to_string(&order) {
                        Ok(payload) => payload,
                        Err(error) => {
                            error!(client = %self.client_id, %error, "Failed to serialize order");
                            continue;
                        }
                    };

                    match stream.send(Message::Text(payload.into())).await {
                        Ok(()) => {
                            info!(client = %self.client_id, price = order.price, size = order.size, side = %order.side, "Order sent to orderbook");
                        },
                        Err(error) => {
                            error!(client = %self.client_id, %error, "Failed to send order to orderbook");
                            break;
                        }
                    }
                }
            }
        }

        if let Err(error) = stream.close(None).await {
            error!(client = %self.client_id, %error, "Failed to close orderbook connection");
        }

        Ok(())
    }
}
