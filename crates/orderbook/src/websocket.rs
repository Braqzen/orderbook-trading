use crate::trade::{Instrument, Order, OrderType, Price, Request};
use eyre::Result;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::{net::SocketAddr, num::NonZeroU64};
use tokio::{
    net::{TcpListener, TcpStream},
    select,
    sync::{mpsc::Sender, oneshot},
    task::{JoinError, JoinSet},
};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};
use uuid::Uuid;

pub struct WsServer {
    socket: SocketAddr,
    request_sender: Sender<Request>,
}

impl WsServer {
    pub fn new(socket: SocketAddr, request_sender: Sender<Request>) -> Self {
        Self {
            socket,
            request_sender,
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
                    let (stream, _) = match accepted {
                        Ok(value) => value,
                        Err(error) => {
                            error!(%error, "Failed to accept WebSocket connection");
                            continue;
                        }
                    };

                    let request_sender = self.request_sender.clone();
                    let connection_token = token.child_token();

                    connections.spawn(Self::handle_connection(
                        stream,
                        request_sender,
                        connection_token,
                    ));
                }
            }
        }

        token.cancel();

        while let Some(result) = connections.join_next().await {
            log_connection_result(result);
        }

        Ok(())
    }

    async fn handle_connection(
        stream: TcpStream,
        request_sender: Sender<Request>,
        token: CancellationToken,
    ) -> Result<()> {
        let mut ws = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(error) => {
                error!(%error, "WebSocket handshake failed");
                return Ok(());
            }
        };

        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                message = ws.next() => {
                    match message {
                        Some(Ok(Message::Text(payload))) => {
                            let order = match serde_json::from_str::<ApiOrder>(&payload) {
                                Ok(order) => order,
                                Err(error) => {
                                    warn!(%error, "Received invalid order");
                                    continue;
                                }
                            };

                            let instrument = order.instrument;
                            let price = match Price::try_from(order.price) {
                                Ok(price) => price,
                                Err(error) => {
                                    warn!(%error, "Received invalid order price");
                                    continue;
                                }
                            };
                            let order = Order::new(order.size, order.side, order.client_id, order.order_id);

                            let (response_sender, response_receiver) = oneshot::channel();

                            let request = Request::new(instrument, price, order, response_sender);

                            if request_sender.send(request).await.is_err() {
                                error!("Order channel closed");
                                break;
                            }

                            let response = match response_receiver.await {
                                Ok(response) => response,
                                Err(error) => {
                                    error!(%error, "Engine dropped order response");
                                    break;
                                }
                            };

                            let payload = match serde_json::to_string(&response) {
                                Ok(payload) => payload,
                                Err(error) => {
                                    error!(%error, "Failed to serialize order response");
                                    continue;
                                }
                            };

                            if let Err(error) = ws.send(Message::Text(payload.into())).await {
                                error!(%error, "Failed to send order response");
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

        if let Err(error) = ws.close(None).await {
            error!(%error, "Failed to close WebSocket connection");
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
    instrument: Instrument,
    price: f64,
    size: NonZeroU64,
    side: OrderType,
    client_id: Uuid,
    order_id: Uuid,
}
