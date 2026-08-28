use crate::{
    api::{Response, order::RawMessage},
    metrics::OrderbookMetrics,
    trade::{Instrument, LimitOrder, ORDER_SIZE_ATOM_STEP, Price, Quantity, Request},
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
    instrument: Instrument,
    /// Channel to send client requests to engine
    order_sender_channel: Sender<Request>,
    /// Token to cancel task
    token: CancellationToken,
    /// Shared connection registry for trade publishing
    connection_registry: ConnectionRegistry,
    metrics: OrderbookMetrics,
}

impl Connection {
    pub fn new(
        stream: TcpStream,
        instrument: Instrument,
        order_sender_channel: Sender<Request>,
        token: CancellationToken,
        connection_registry: ConnectionRegistry,
        metrics: OrderbookMetrics,
    ) -> Self {
        Self {
            stream,
            instrument,
            order_sender_channel,
            token,
            connection_registry,
            metrics,
        }
    }

    pub async fn run(self) -> Result<()> {
        let mut ws_stream = match accept_async(self.stream).await {
            Ok(stream) => stream,
            Err(error) => {
                error!(instrument = %self.instrument, %error, "WebSocket handshake failed");
                return Ok(());
            }
        };

        let (client_order_sender, mut client_order_receiver) = channel::<Request>(ORDER_QUEUE);
        let (outbound_sender_channel, mut outbound_receiver_channel) =
            channel::<Response>(OUTBOUND_QUEUE_SIZE);
        let mut client_id = None;
        self.metrics.client_connected();

        loop {
            select! {
                _ = self.token.cancelled() => break,

                // Receive data over socket from connected client and send it to our order channel to bound the number of orders
                message = ws_stream.next() => {
                    match message {
                        Some(Ok(Message::Text(payload))) => {
                            let raw_message = match serde_json::from_str::<RawMessage>(&payload) {
                                Ok(message) => message,
                                Err(error) => {
                                    warn!(instrument = %self.instrument, %error, "Received invalid message");
                                    continue;
                                }
                            };

                            let Some(request) = parse_raw_message(
                                &self.instrument,
                                raw_message,
                                &mut client_id,
                                &self.connection_registry,
                                outbound_sender_channel.clone(),
                                self.token.clone(),
                            )
                            .await
                            else {
                                continue;
                            };

                            let request_client_id = request.client_id();
                            self.metrics.client_order_enqueued(request_client_id);
                            match client_order_sender.try_send(request) {
                                Ok(()) => {}
                                Err(TrySendError::Closed(_)) => {
                                    self.metrics.client_order_dequeued(request_client_id);
                                    error!(instrument = %self.instrument, "Order channel closed");
                                    break;
                                }
                                Err(TrySendError::Full(_)) => {
                                    self.metrics.client_order_dequeued(request_client_id);
                                    warn!(instrument = %self.instrument, "Client order queue full");
                                    break;
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Err(error)) => {
                            error!(instrument = %self.instrument, %error, "WebSocket connection failed");
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
                    let request_client_id = request.client_id();
                    self.metrics.client_order_dequeued(request_client_id);
                    self.metrics.global_order_enqueued();

                    match self.order_sender_channel.try_send(request) {
                        Ok(()) => {}
                        Err(TrySendError::Closed(_)) => {
                            self.metrics.global_order_dequeued();
                            error!(instrument = %self.instrument, "Order channel closed");
                            break;
                        }
                        Err(TrySendError::Full(_)) => {
                            self.metrics.global_order_dequeued();
                            warn!(instrument = %self.instrument, "Global order queue full");
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
                            error!(instrument = %self.instrument, %error, "Failed to serialize outbound message");
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
                            error!(instrument = %self.instrument, %error, "Failed to send WebSocket message");
                            break;
                        }
                        Err(_) => {
                            warn!(instrument = %self.instrument, "Client send timed out");
                            break;
                        }
                    }
                }
            }
        }

        // Registry cleanup, remove client upon connection/task closure
        if let Some(client_id) = client_id {
            let queued_orders = client_order_sender.max_capacity() - client_order_sender.capacity();
            if queued_orders > 0 {
                self.metrics.client_orders_dropped(client_id, queued_orders);
            }

            let mut registry = self.connection_registry.write().await;

            let should_remove = match registry.get(&client_id) {
                Some(client) => client.same_channel(&outbound_sender_channel),
                None => false,
            };

            if should_remove {
                registry.remove(&client_id);
            }
        }

        self.metrics.client_disconnected();

        if let Err(error) = ws_stream.close(None).await {
            error!(instrument = %self.instrument, %error, "Failed to close WebSocket connection");
        }

        Ok(())
    }
}

async fn parse_raw_message(
    connection_instrument: &Instrument,
    raw_message: RawMessage,
    client_id: &mut Option<Uuid>,
    connection_registry: &ConnectionRegistry,
    outbound_sender_channel: Sender<Response>,
    token: CancellationToken,
) -> Option<Request> {
    match raw_message {
        RawMessage::Place {
            instrument,
            price,
            size,
            side,
            client_id: submitted_client_id,
            order_id,
        } => {
            if size.get() % ORDER_SIZE_ATOM_STEP != 0 {
                warn!(
                    instrument = %connection_instrument,
                    size = size.get(),
                    "Order size must use at most six decimal places"
                );
                return None;
            }

            if !register_client(
                client_id,
                submitted_client_id,
                connection_instrument,
                connection_registry,
                outbound_sender_channel,
                token,
            )
            .await
            {
                return None;
            }

            let order = LimitOrder::new(
                Quantity::from(size.get()),
                side,
                submitted_client_id,
                order_id,
            );

            Some(Request::Place {
                instrument,
                price: Price::from(price.get()),
                order,
            })
        }
        RawMessage::Cancel {
            client_id: submitted_client_id,
            order_id,
            price,
            side,
        } => {
            if !register_client(
                client_id,
                submitted_client_id,
                connection_instrument,
                connection_registry,
                outbound_sender_channel,
                token,
            )
            .await
            {
                return None;
            }

            Some(Request::Cancel {
                client_id: submitted_client_id,
                order_id,
                price: Price::from(price.get()),
                side,
            })
        }
    }
}

async fn register_client(
    client_id: &mut Option<Uuid>,
    submitted_client_id: Uuid,
    connection_instrument: &Instrument,
    connection_registry: &ConnectionRegistry,
    outbound_sender_channel: Sender<Response>,
    token: CancellationToken,
) -> bool {
    // TODO: should move into a login/register message sequence
    match client_id {
        Some(registered_id) if *registered_id != submitted_client_id => {
            warn!(
                instrument = %connection_instrument,
                registered_client = %registered_id,
                submitted_client = %submitted_client_id,
                "Connection attempted to change client ID"
            );
            false
        }
        Some(_) => true,
        None => {
            connection_registry.write().await.insert(
                submitted_client_id,
                ClientHandle::new(outbound_sender_channel, token),
            );
            *client_id = Some(submitted_client_id);
            true
        }
    }
}
