use crate::{
    api::{
        LoginRejectionReason, Response,
        order::{ClientMessage, LoginRequest, RawMessage},
        session::SessionStore,
    },
    metrics::OrderbookMetrics,
    trade::{Instrument, LimitOrder, ORDER_SIZE_ATOM_STEP, Price, Quantity, Request},
};
use eyre::Result;
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::{
    net::TcpStream,
    select,
    sync::mpsc::{Sender, channel, error::TrySendError},
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

pub struct Connection {
    /// Client stream
    stream: TcpStream,
    instrument: Instrument,
    /// Channel to send client requests to engine
    order_sender_channel: Sender<Request>,
    /// Token to cancel task
    token: CancellationToken,
    /// Shared connection registry for trade publishing
    sessions: SessionStore,
    metrics: OrderbookMetrics,
}

impl Connection {
    pub fn new(
        stream: TcpStream,
        instrument: Instrument,
        order_sender_channel: Sender<Request>,
        token: CancellationToken,
        sessions: SessionStore,
        metrics: OrderbookMetrics,
    ) -> Self {
        Self {
            stream,
            instrument,
            order_sender_channel,
            token,
            sessions,
            metrics,
        }
    }

    pub async fn run(self) -> Result<()> {
        let Self {
            stream,
            instrument,
            order_sender_channel,
            token,
            sessions,
            metrics,
        } = self;

        let mut ws_stream = match accept_async(stream).await {
            Ok(stream) => stream,
            Err(error) => {
                error!(%instrument, %error, "WebSocket handshake failed");
                return Ok(());
            }
        };

        let (client_order_sender, mut client_order_receiver) = channel::<Request>(ORDER_QUEUE);
        let (outbound_sender_channel, mut outbound_receiver_channel) =
            channel::<Response>(OUTBOUND_QUEUE_SIZE);
        let mut client_id = None;
        metrics.client_connected();

        loop {
            select! {
                _ = token.cancelled() => break,

                // Receive data over socket from connected client and send it to our order channel
                // to bound the number of orders they can send us for processing
                client_message = ws_stream.next() => {
                    match client_message {
                        Some(Ok(Message::Text(payload))) => {
                            let message = match serde_json::from_str::<ClientMessage>(&payload) {
                                Ok(message) => message,
                                Err(error) => {
                                    warn!(%instrument, %error, ?client_id, "Received invalid message");
                                    continue;
                                }
                            };

                            match message {
                                ClientMessage::Login(login) => {
                                    process_login(
                                        &instrument,
                                        &sessions,
                                        &token,
                                        &login,
                                        &mut client_id,
                                        &outbound_sender_channel,
                                    )
                                    .await;
                                }
                                ClientMessage::Trading(message) => {
                                    match process_order(
                                        &instrument,
                                        &sessions,
                                        &metrics,
                                        message,
                                        client_id,
                                        &outbound_sender_channel,
                                        &client_order_sender,
                                    )
                                    .await
                                    {
                                        ProcessOrderOutcome::Continue => {}
                                        ProcessOrderOutcome::CloseConnection => break,
                                    }
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Err(error)) => {
                            error!(%instrument, %error, ?client_id, "WebSocket connection failed");
                            break;
                        }
                        _ => {}
                    }
                }

                // Orders are queued and bounded by channel, forward to engine channel for trading
                engine_request = client_order_receiver.recv() => {
                    let Some(request) = engine_request else {
                        break;
                    };

                    let request_client_id = request.client_id();
                    metrics.client_order_dequeued(request_client_id);

                    match order_sender_channel.try_send(request) {
                        Ok(()) => metrics.global_order_enqueued(),
                        Err(TrySendError::Closed(_)) => {
                            error!(%instrument, ?client_id, "Order channel closed");
                            break;
                        }
                        Err(TrySendError::Full(_)) => {
                            warn!(%instrument, ?client_id, "Global order queue full");
                            break;
                        }
                    }
                }

                // Receive messages from the engine and attempt to publish to connected client
                engine_response = outbound_receiver_channel.recv() => {
                    let Some(response) = engine_response else {
                        break;
                    };

                    let payload = match serde_json::to_string(&response) {
                        Ok(payload) => payload,
                        Err(error) => {
                            error!(%instrument, %error, ?client_id, "Failed to serialize outbound message");
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
                            error!(%instrument, %error, ?client_id, "Failed to send WebSocket message");
                            break;
                        }
                        Err(_) => {
                            warn!(%instrument, ?client_id, "Client send timed out");
                            break;
                        }
                    }
                }
            }
        }

        // TODO: may want to attempt to msg client before dropping connection to indicate pending messages
        // Cleanup, remove client upon connection/task closure
        if let Some(client_id) = client_id {
            let queued_orders = client_order_sender.max_capacity() - client_order_sender.capacity();
            if queued_orders > 0 {
                metrics.client_orders_dropped(client_id, queued_orders);
            }

            sessions.logout(client_id, &outbound_sender_channel).await;
        }

        metrics.client_disconnected();

        if let Err(error) = ws_stream.close(None).await {
            error!(%instrument, %error, ?client_id, "Failed to close WebSocket connection");
        }

        Ok(())
    }
}

async fn process_login(
    instrument: &Instrument,
    sessions: &SessionStore,
    token: &CancellationToken,
    login: &LoginRequest,
    client_id: &mut Option<Uuid>,
    outbound_sender_channel: &Sender<Response>,
) {
    match sessions
        .login(login, outbound_sender_channel.clone(), token.clone())
        .await
    {
        Ok(()) => match outbound_sender_channel.try_send(Response::LoginAccepted) {
            Ok(()) => *client_id = Some(login.client_id),
            Err(_) => {
                warn!(
                    %instrument,
                    client_id = %login.client_id,
                    "Failed to send login acceptance"
                );

                sessions
                    .logout(login.client_id, outbound_sender_channel)
                    .await;
            }
        },
        Err(reason) => {
            warn!(
                %instrument,
                client_id = %login.client_id,
                ?reason,
                "Orderbook login rejected"
            );

            if outbound_sender_channel
                .try_send(Response::login_rejected(reason))
                .is_err()
            {
                warn!(
                    %instrument,
                    client_id = %login.client_id,
                    "Failed to send login rejection"
                );
            }
        }
    }
}

#[derive(PartialEq, Eq)]
enum ProcessOrderOutcome {
    Continue,
    CloseConnection,
}

async fn process_order(
    instrument: &Instrument,
    sessions: &SessionStore,
    metrics: &OrderbookMetrics,
    message: RawMessage,
    client_id: Option<Uuid>,
    outbound_sender_channel: &Sender<Response>,
    client_order_sender: &Sender<Request>,
) -> ProcessOrderOutcome {
    let Some(client_id) = client_id else {
        warn!(
            %instrument,
            ?client_id,
            "Trading message received before login"
        );

        if outbound_sender_channel
            .try_send(Response::login_rejected(LoginRejectionReason::NotLoggedIn))
            .is_err()
        {
            warn!(%instrument, "Failed to send login rejection");
        }

        return ProcessOrderOutcome::Continue;
    };

    if let Err(reason) = sessions.verify_auth(client_id).await {
        warn!(
            %instrument,
            %client_id,
            ?reason,
            "Client session no longer registered"
        );

        if outbound_sender_channel
            .try_send(Response::login_rejected(reason))
            .is_err()
        {
            warn!(%instrument, "Failed to send login rejection");
        }

        return ProcessOrderOutcome::Continue;
    }

    let Some(request) = create_trade_request(instrument, client_id, message) else {
        return ProcessOrderOutcome::Continue;
    };

    match client_order_sender.try_send(request) {
        Ok(()) => {
            metrics.client_order_enqueued(client_id);
            ProcessOrderOutcome::Continue
        }
        Err(TrySendError::Closed(_)) => {
            error!(%instrument, %client_id, "Order channel closed");
            ProcessOrderOutcome::CloseConnection
        }
        Err(TrySendError::Full(_)) => {
            warn!(%instrument, %client_id, "Client order queue full");
            ProcessOrderOutcome::CloseConnection
        }
    }
}

fn create_trade_request(
    connection_instrument: &Instrument,
    client_id: Uuid,
    raw_message: RawMessage,
) -> Option<Request> {
    match raw_message {
        RawMessage::Place {
            instrument,
            price,
            size,
            side,
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

            let order = LimitOrder::new(Quantity::from(size.get()), side, client_id, order_id);

            Some(Request::Place {
                instrument,
                price: Price::from(price.get()),
                order,
            })
        }
        RawMessage::Cancel {
            order_id,
            price,
            side,
        } => Some(Request::Cancel {
            client_id,
            order_id,
            price: Price::from(price.get()),
            side,
        }),
    }
}
