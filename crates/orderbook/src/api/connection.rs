use crate::{
    api::{Response, order::RawOrder},
    trade::{LimitOrder, Price, Request},
};
use eyre::Result;
use futures_util::{SinkExt, StreamExt};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    net::TcpStream,
    select,
    sync::{
        RwLock,
        mpsc::{Sender, channel, error::TrySendError},
    },
    time::timeout,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};
use uuid::Uuid;

/// Number of orders a client can send to the server to buffer
const ORDER_QUEUE: usize = 16;
/// Number of messages that can be made in the engine and buffered before publishing
const OUTBOUND_QUEUE_SIZE: usize = 1024;
/// Quantity of time before a connection is dropped when publishing
const OUTBOUND_SEND_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct ClientHandle {
    sender: Sender<Response>,
    token: CancellationToken,
}

impl ClientHandle {
    pub fn new(sender: Sender<Response>, token: CancellationToken) -> Self {
        Self { sender, token }
    }

    pub fn try_send(&self, message: Response) -> Result<(), TrySendError<Response>> {
        self.sender.try_send(message)
    }

    pub fn disconnect(&self) {
        self.token.cancel();
    }

    pub fn same_channel(&self, other: &Sender<Response>) -> bool {
        self.sender.same_channel(other)
    }
}

pub type ConnectionRegistry = Arc<RwLock<HashMap<Uuid, ClientHandle>>>;

pub struct Connection {
    /// Client stream
    stream: TcpStream,
    /// Channel to send client requests to engine
    order_sender_channel: Sender<Request>,
    /// Token to cancel task
    token: CancellationToken,
    /// Shared connection registry for trade publishing
    connection_registry: ConnectionRegistry,
}

impl Connection {
    pub fn new(
        stream: TcpStream,
        order_sender_channel: Sender<Request>,
        token: CancellationToken,
        connection_registry: ConnectionRegistry,
    ) -> Self {
        Self {
            stream,
            order_sender_channel,
            token,
            connection_registry,
        }
    }

    pub async fn run(self) -> Result<()> {
        let mut ws_stream = match accept_async(self.stream).await {
            Ok(stream) => stream,
            Err(error) => {
                error!(%error, "WebSocket handshake failed");
                return Ok(());
            }
        };

        let (client_order_sender, mut client_order_receiver) = channel::<Request>(ORDER_QUEUE);
        let (outbound_sender_channel, mut outbound_receiver_channel) =
            channel::<Response>(OUTBOUND_QUEUE_SIZE);
        let mut client_id = None;

        loop {
            select! {
                _ = self.token.cancelled() => break,

                // Receive data over socket from connected client and send it to our order channel to bound the number of orders
                message = ws_stream.next() => {
                    match message {
                        Some(Ok(Message::Text(payload))) => {
                            let raw_order = match serde_json::from_str::<RawOrder>(&payload) {
                                Ok(order) => order,
                                Err(error) => {
                                    warn!(%error, "Received invalid order");
                                    continue;
                                }
                            };

                            let instrument = raw_order.instrument;
                            let price = match Price::try_from(raw_order.price) {
                                Ok(price) => price,
                                Err(error) => {
                                    warn!(%error, "Received invalid order price");
                                    continue;
                                }
                            };

                            // TODO: should move into a login/register message sequence
                            match client_id {
                                Some(registered_id)
                                    if registered_id != raw_order.client_id =>
                                {
                                    warn!(
                                        registered_client = %registered_id,
                                        submitted_client = %raw_order.client_id,
                                        "Connection attempted to change client ID"
                                    );
                                    continue;
                                }
                                Some(_) => {}
                                None => {
                                    self.connection_registry.write().await.insert(
                                        raw_order.client_id,
                                        ClientHandle::new(
                                            outbound_sender_channel.clone(),
                                            self.token.clone(),
                                        ),
                                    );
                                    client_id = Some(raw_order.client_id);
                                }
                            }

                            let order = LimitOrder::new(raw_order.size, raw_order.side, raw_order.client_id, raw_order.order_id);
                            let request = Request::new(instrument, price, order);

                            match client_order_sender.try_send(request) {
                                Ok(()) => {}
                                Err(TrySendError::Closed(_)) => {
                                    error!("Order channel closed");
                                    break;
                                }
                                Err(TrySendError::Full(_)) => {
                                    warn!("Client order queue full");
                                    break;
                                }
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

                // Orders are queued and bounded by channel, forward to engine channel for trading
                request = client_order_receiver.recv() => {
                    let Some(request) = request else {
                        break;
                    };
                    match self.order_sender_channel.try_send(request) {
                        Ok(()) => {}
                        Err(TrySendError::Closed(_)) => {
                            error!("Order channel closed");
                            break;
                        }
                        Err(TrySendError::Full(_)) => {
                            warn!("Global order queue full");
                            break;
                        }
                    }
                }

                // Receive messages from the engine and attempt to publish to connected client
                message = outbound_receiver_channel.recv() => {
                    let Some(message) = message else {
                        break;
                    };

                    let payload = match serde_json::to_string(&message) {
                        Ok(payload) => payload,
                        Err(error) => {
                            error!(%error, "Failed to serialize outbound message");
                            continue;
                        }
                    };

                    match timeout(
                        OUTBOUND_SEND_TIMEOUT,
                        ws_stream.send(Message::Text(payload.into())),
                    )
                    .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => {
                            error!(%error, "Failed to send WebSocket message");
                            break;
                        }
                        Err(_) => {
                            warn!("Client send timed out");
                            break;
                        }
                    }
                }
            }
        }

        // Registry cleanup, remove client upon connection/task closure
        if let Some(client_id) = client_id {
            let mut registry = self.connection_registry.write().await;

            let should_remove = match registry.get(&client_id) {
                Some(client) => client.same_channel(&outbound_sender_channel),
                None => false,
            };

            if should_remove {
                registry.remove(&client_id);
            }
        }

        if let Err(error) = ws_stream.close(None).await {
            error!(%error, "Failed to close WebSocket connection");
        }

        Ok(())
    }
}
