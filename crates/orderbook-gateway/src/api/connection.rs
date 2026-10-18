//! Central handler for one client on this instrument.
//! It receives client messages, queues orders for the orderbook, returns responses, and cleans up the
//! session when the connection ends.

use crate::{
    api::{
        ClientMessage, LoginRejectionReason, LoginRequest, OrderRejection, RawMessage, Response,
        SessionStore,
    },
    execution::ExecutionReader,
    metrics::GatewayMetrics,
    submit::Submitter,
    trade::RejectionReason,
    trade::{Instrument, LimitOrder, Price, Quantity, Request},
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
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};
use uuid::Uuid;

/// Number of orders a client can send to the server to buffer
const ORDER_QUEUE: usize = 16;
/// Number of messages that can be made in the engine and buffered before publishing
///
/// Significantly larger that [`ORDER_QUEUE`] because we may need to queue many trade responses
const OUTBOUND_QUEUE_SIZE: usize = 1024;
/// Quantity of time before a connection is dropped when publishing
const OUTBOUND_SEND_TIMEOUT: Duration = Duration::from_secs(2);

pub struct Connection {
    /// Client stream
    stream: Option<TcpStream>,
    /// Used for logging
    instrument: Instrument,
    /// Sends client requests to the orderbook
    submitter: Submitter,
    /// Reads this client's executions from Redpanda
    executions: ExecutionReader,
    /// Token to cancel task
    token: CancellationToken,
    /// Shared connection registry for trade publishing
    sessions: SessionStore,
    metrics: GatewayMetrics,
}

impl Connection {
    pub fn new(
        stream: TcpStream,
        instrument: Instrument,
        submitter: Submitter,
        executions: ExecutionReader,
        token: CancellationToken,
        sessions: SessionStore,
        metrics: GatewayMetrics,
    ) -> Self {
        Self {
            stream: Some(stream),
            instrument,
            submitter,
            executions,
            token,
            sessions,
            metrics,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        let Some(stream) = self.stream.take() else {
            error!(instrument = %self.instrument, "Client stream missing");
            return Ok(());
        };
        let mut ws_stream = match accept_async(stream).await {
            Ok(stream) => stream,
            Err(error) => {
                error!(instrument = %self.instrument, %error, "WebSocket handshake failed");
                return Ok(());
            }
        };
        let (client_order_sender, mut client_order_receiver) = channel::<Request>(ORDER_QUEUE);
        let (outbound_sender_channel, mut outbound_receiver_channel) =
            channel::<Response>(OUTBOUND_QUEUE_SIZE);
        let (reader_done_sender, mut reader_done_receiver) = channel::<Result<()>>(1);

        let mut client_id = None;
        let reader_token = self.token.child_token();
        self.metrics.client_connected();

        loop {
            select! {
                _ = self.token.cancelled() => break,

                // Receive data over socket from connected client and send it to our order channel
                // to bound the number of orders they can send us for processing
                client_message = ws_stream.next() => {
                    match client_message {
                        Some(Ok(Message::Text(payload))) => {
                            let message = match serde_json::from_str::<ClientMessage>(&payload) {
                                Ok(message) => message,
                                Err(error) => {
                                    warn!(instrument = %self.instrument, %error, ?client_id, "Received invalid message");
                                    continue;
                                }
                            };

                            match message {
                                ClientMessage::Login(login) => {
                                    self.login(
                                        &login,
                                        &mut client_id,
                                        &outbound_sender_channel,
                                        &reader_token,
                                        &reader_done_sender,
                                    )
                                    .await;
                                }
                                ClientMessage::Trading(message) => {
                                    match self.enqueue_order(
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
                            error!(instrument = %self.instrument, %error, ?client_id, "WebSocket connection failed");
                            break;
                        }
                        _ => {}
                    }
                }

                // Orders are queued and bounded by channel for this connection
                // Forward requests to the orderbook for trading
                engine_request = client_order_receiver.recv() => {
                    let Some(request) = engine_request else {
                        break;
                    };

                    // SAFETY: Invariant, can only be used here. 1st select branch sets the ID after login
                    //         trades rejected if not logged in so safe here
                    let Some(id) = client_id else {
                        error!(instrument = %self.instrument, "Order forwarded before login");
                        break;
                    };

                    self.metrics.client_order_dequeued(id);

                    if let Err(error) = self.submitter.submit(request).await {
                        error!(instrument = %self.instrument, client_id = %id, %error, "Failed to submit order");
                        break;
                    }
                }

                // Receive messages from the execution log and attempt to publish to connected client
                engine_response = outbound_receiver_channel.recv() => {
                    let Some(response) = engine_response else {
                        break;
                    };

                    if self.publish(&mut ws_stream, client_id, &response).await.is_err() {
                        break;
                    }
                }

                reader = reader_done_receiver.recv(), if client_id.is_some() => {
                    match reader {
                        Some(Ok(())) => {}
                        Some(Err(error)) => {
                            error!(instrument = %self.instrument, ?client_id, %error, "Execution reader failed");
                        }
                        None => {
                            error!(instrument = %self.instrument, ?client_id, "Execution reader stopped");
                        }
                    }
                    break;
                }
            }
        }

        reader_token.cancel();

        // TODO: may want to attempt to msg client before dropping connection to indicate pending messages
        // Cleanup, remove client upon connection/task closure
        if let Some(client_id) = client_id {
            let queued_orders = client_order_sender.max_capacity() - client_order_sender.capacity();
            if queued_orders > 0 {
                self.metrics.client_orders_dropped(client_id, queued_orders);
            }

            self.sessions
                .logout(client_id, &outbound_sender_channel)
                .await;
        }

        self.metrics.client_disconnected();

        if let Err(error) = ws_stream.close(None).await {
            error!(instrument = %self.instrument, %error, ?client_id, "Failed to close WebSocket connection");
        }

        Ok(())
    }

    async fn login(
        &self,
        login: &LoginRequest,
        client_id: &mut Option<Uuid>,
        outbound_sender_channel: &Sender<Response>,
        reader_token: &CancellationToken,
        reader_done_sender: &Sender<Result<()>>,
    ) {
        match self
            .sessions
            .login(login, outbound_sender_channel.clone(), self.token.clone())
            .await
        {
            Ok(()) => {
                let Some(record) = self.sessions.client_record(login.client_id).await else {
                    warn!(
                        instrument = %self.instrument,
                        client_id = %login.client_id,
                        "Logged in session was not stored"
                    );
                    return;
                };

                match record.try_send(Response::LoginAccepted) {
                    Ok(()) => {
                        *client_id = Some(login.client_id);
                        let executions = self.executions.clone();
                        let outbound = record.outbound();
                        let reader_token = reader_token.clone();
                        let done = reader_done_sender.clone();
                        let logged_in = login.client_id;
                        let last_exec_seq = record.last_exec_seq();
                        tokio::spawn(async move {
                            let result = executions
                                .run(logged_in, last_exec_seq, outbound, reader_token)
                                .await;
                            if done.send(result).await.is_err() {
                                warn!(client_id = %logged_in, "Execution reader finished after the connection closed");
                            }
                        });
                    }
                    Err(_) => {
                        warn!(
                            instrument = %self.instrument,
                            client_id = %login.client_id,
                            "Failed to send login acceptance"
                        );

                        self.sessions
                            .logout(login.client_id, outbound_sender_channel)
                            .await;
                    }
                }
            }
            Err(reason) => {
                warn!(
                    instrument = %self.instrument,
                    client_id = %login.client_id,
                    ?reason,
                    "Orderbook login rejected"
                );

                if outbound_sender_channel
                    .try_send(Response::login_rejected(reason))
                    .is_err()
                {
                    warn!(
                        instrument = %self.instrument,
                        client_id = %login.client_id,
                        "Failed to send login rejection"
                    );
                }
            }
        }
    }

    async fn enqueue_order(
        &self,
        message: RawMessage,
        client_id: Option<Uuid>,
        outbound_sender_channel: &Sender<Response>,
        client_order_sender: &Sender<Request>,
    ) -> ProcessOrderOutcome {
        let Some(client_id) = client_id else {
            warn!(
                instrument = %self.instrument,
                ?client_id,
                "Trading message received before login"
            );

            if outbound_sender_channel
                .try_send(Response::login_rejected(LoginRejectionReason::NotLoggedIn))
                .is_err()
            {
                warn!(instrument = %self.instrument, "Failed to send login rejection");
            }

            return ProcessOrderOutcome::Continue;
        };

        if let Err(reason) = self.sessions.verify_auth(client_id).await {
            warn!(
                instrument = %self.instrument,
                %client_id,
                ?reason,
                "Client session no longer registered"
            );

            if outbound_sender_channel
                .try_send(Response::login_rejected(reason))
                .is_err()
            {
                warn!(instrument = %self.instrument, "Failed to send login rejection");
            }

            return ProcessOrderOutcome::Continue;
        }

        let request = match self.create_request(client_id, message) {
            Ok(request) => request,
            Err(rejection) => {
                warn!(
                    instrument = %self.instrument,
                    %client_id,
                    order = %rejection.order_id,
                    ?rejection.reason,
                    "Order rejected at connection boundary"
                );

                if outbound_sender_channel
                    .try_send(Response::OrderRejected(rejection))
                    .is_err()
                {
                    warn!(instrument = %self.instrument, %client_id, "Failed to send order rejection");
                }

                return ProcessOrderOutcome::Continue;
            }
        };

        match client_order_sender.try_send(request) {
            Ok(()) => {
                self.metrics.client_order_enqueued(client_id);
                ProcessOrderOutcome::Continue
            }
            Err(TrySendError::Closed(_)) => {
                error!(instrument = %self.instrument, %client_id, "Order channel closed");
                ProcessOrderOutcome::CloseConnection
            }
            Err(TrySendError::Full(_)) => {
                warn!(instrument = %self.instrument, %client_id, "Client order queue full");
                ProcessOrderOutcome::CloseConnection
            }
        }
    }

    fn create_request(
        &self,
        client_id: Uuid,
        raw_message: RawMessage,
    ) -> Result<Request, OrderRejection> {
        match raw_message {
            RawMessage::Place {
                instrument,
                price,
                size,
                side,
                order_id,
            } => {
                let price = Price::from(price);
                let order_size = Quantity::from(size);

                if !order_size.valid_size() {
                    return Err(OrderRejection {
                        exec_seq: 0,
                        order_id,
                        instrument,
                        price,
                        size: order_size,
                        side,
                        reason: RejectionReason::InvalidOrderSize,
                    });
                }

                if instrument != self.instrument {
                    return Err(OrderRejection {
                        exec_seq: 0,
                        order_id,
                        instrument,
                        price,
                        size: order_size,
                        side,
                        reason: RejectionReason::InvalidInstrument,
                    });
                }

                let order = LimitOrder::new(order_size, side, client_id, order_id);

                Ok(Request::Place {
                    instrument: self.instrument.clone(),
                    price,
                    order,
                })
            }
            RawMessage::Cancel {
                order_id,
                price,
                side,
            } => Ok(Request::Cancel {
                client_id,
                order_id,
                price: Price::from(price),
                side,
            }),
        }
    }

    async fn publish(
        &self,
        ws_stream: &mut WebSocketStream<TcpStream>,
        client_id: Option<Uuid>,
        response: &Response,
    ) -> Result<(), ()> {
        let payload = match serde_json::to_string(response) {
            Ok(payload) => payload,
            Err(error) => {
                error!(instrument = %self.instrument, %error, ?client_id, "Failed to serialize outbound message");
                return Ok(());
            }
        };

        match timeout(
            OUTBOUND_SEND_TIMEOUT,
            ws_stream.send(Message::Text(payload.into())),
        )
        .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                error!(instrument = %self.instrument, %error, ?client_id, "Failed to send WebSocket message");
                Err(())
            }
            Err(_) => {
                warn!(instrument = %self.instrument, ?client_id, "Client send timed out");
                Err(())
            }
        }
    }
}

#[derive(PartialEq, Eq)]
enum ProcessOrderOutcome {
    Continue,
    CloseConnection,
}
