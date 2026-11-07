//! The orderbook is the outbound api boundary for one instrument.
//!
//! One task owns the instrument's websocket session and reconnect lifecycle.
//! It receives engine requests while logged in and forwards orderbook responses back to the engine.
//!
//! Connect or login failure retries after 1s, doubling to 60s; login resets the delay to 1s.
//! Ten consecutive retries at 60s stop it.
//! Once retries stop, the instrument remains unavailable and the engine skips trading it.

mod backoff;

use crate::{
    api::{
        ExecutionReport, Response, SubmitOutcome, WsUrl,
        orderbook::{
            LoginRequest, LoginResponse, OrderBookRegistry, Request, RequestKind, RequestMetadata,
            book::backoff::ReconnectBackoff,
        },
    },
    trade::Instrument,
    valkey::SequenceCache,
};
use eyre::{Result, eyre};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::{
    net::TcpStream,
    select,
    sync::mpsc::{Receiver, Sender, error::TrySendError},
    time::{sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Quantity of time before a connection is dropped
const OUTBOUND_SEND_TIMEOUT: Duration = Duration::from_secs(2);
/// Number of orders waiting to be sent to the orderbook
const ORDER_QUEUE: usize = 128;

pub struct OrderBook {
    /// Random client ID
    client_id: Uuid,
    /// Instrument handled by this orderbook task
    instrument: Instrument,
    /// Connect to the orderbook via websocket
    url: WsUrl,
    /// Receive responses from the orderbook and forward to engine
    response_sender_channel: Sender<ExecutionReport>,
    /// Notifies the engine when a place request reached the gateway websocket
    submit_outcome_channel: Sender<SubmitOutcome>,
    /// Reads the execution sequence before each login
    sequences: SequenceCache,
    /// Routes engine requests to this task while it is logged in
    orderbook_registry: OrderBookRegistry,
}

impl OrderBook {
    pub fn new(
        client_id: Uuid,
        instrument: Instrument,
        url: WsUrl,
        response_sender_channel: Sender<ExecutionReport>,
        submit_outcome_channel: Sender<SubmitOutcome>,
        sequences: SequenceCache,
        orderbook_registry: OrderBookRegistry,
    ) -> Self {
        Self {
            client_id,
            instrument,
            url,
            response_sender_channel,
            submit_outcome_channel,
            sequences,
            orderbook_registry,
        }
    }

    pub async fn run(mut self, token: CancellationToken) -> Result<()> {
        let mut backoff = ReconnectBackoff::new();

        loop {
            let error = match self.start_session(&token, &mut backoff).await {
                Ok(()) => return Ok(()),
                Err(_) if token.is_cancelled() => return Ok(()),
                Err(error) => error,
            };

            warn!(client = %self.client_id, instrument = %self.instrument, %error, "Orderbook session failed");

            // Orderbook appears dead because we've tried repeatedly for many minutes, kill this task, nothing respawns it
            let Some(retry_delay) = backoff.retry_delay() else {
                warn!(
                    client = %self.client_id,
                    instrument = %self.instrument,
                    "Stopped reconnecting to orderbook after repeated failures"
                );
                return Ok(());
            };

            select! {
                biased;

                _ = token.cancelled() => return Ok(()),
                _ = sleep(retry_delay) => {}
            }
        }
    }

    async fn start_session(
        &mut self,
        token: &CancellationToken,
        backoff: &mut ReconnectBackoff,
    ) -> Result<()> {
        let (mut stream, _response) = select! {
            biased;

            _ = token.cancelled() => return Ok(()),
            result = connect_async(self.url.as_str()) => {
                match result {
                    Ok(connection) => connection,
                    Err(error) => return Err(eyre!(error)),
                }
            },
        };

        info!(
            client = %self.client_id,
            instrument = %self.instrument,
            url = %self.url,
            "Connected to orderbook"
        );

        // TODO: messy before&after login handling, clean up in proper issue
        let last_exec_seq = match self
            .sequences
            .last_exec_seq(self.client_id, &self.instrument)
            .await
        {
            Ok(sequence) => sequence,
            Err(error) => return Err(error),
        };

        if let Err(error) = self.login(&mut stream, last_exec_seq, token).await {
            if let Err(error) = stream.close(None).await {
                error!(client = %self.client_id, instrument = %self.instrument, %error, "Failed to close orderbook connection");
            }
            return Err(error);
        }
        backoff.reset();

        let (order_sender_channel, mut order_receiver_channel) =
            tokio::sync::mpsc::channel(ORDER_QUEUE);

        // The engine looks up this sender by instrument and forwards requests into this session.
        self.orderbook_registry
            .register(self.instrument.clone(), order_sender_channel.clone())
            .await;

        let session_error = loop {
            select! {
                biased;

                _ = token.cancelled() => break None,

                // Connected orderbook has responded, forward to engine
                orderbook_service_message = stream.next() => {
                    match orderbook_service_message {
                        Some(Ok(Message::Text(payload))) => {
                            let response = match serde_json::from_str::<Response>(&payload) {
                                Ok(response) => response,
                                Err(error) => {
                                    warn!(client = %self.client_id, instrument = %self.instrument, %error, "Received invalid response");
                                    continue;
                                }
                            };

                            match self.response_sender_channel.try_send(ExecutionReport {
                                instrument: self.instrument.clone(),
                                response,
                            }) {
                                Ok(()) => {}
                                Err(TrySendError::Closed(_)) => {
                                    error!(client = %self.client_id, instrument = %self.instrument, "Engine response channel closed");
                                    break None;
                                }
                                Err(TrySendError::Full(_)) => {
                                    warn!(client = %self.client_id, instrument = %self.instrument, "Engine response queue full");
                                    break None;
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            error!(client = %self.client_id, instrument = %self.instrument, "Orderbook service explicitly closed connection");
                            break Some(eyre!("Orderbook service closed connection"));
                        }
                        Some(Err(error)) => {
                            error!(client = %self.client_id, instrument = %self.instrument, %error, "Unknown error");
                            break Some(eyre!(error));
                        }
                        None => {
                            error!(client = %self.client_id, instrument = %self.instrument, "Disconnected from orderbook service");
                            break Some(eyre!("Disconnected from orderbook service"));
                        }
                        _ => {
                            warn!(client = %self.client_id, instrument = %self.instrument, "Skipping unexpected message");
                        }
                    }
                }

                // Received request from engine to forward to the orderbook service
                request = order_receiver_channel.recv() => {
                    let Some(request) = request else {
                        error!(client = %self.client_id, instrument = %self.instrument, "Orderbook order channel closed");
                        break None;
                    };

                    if let Err(error) = self.send_request(&mut stream, &request).await {
                        break Some(error);
                    }
                }
            }
        };

        order_receiver_channel.close();
        self.clear_route(&order_sender_channel).await;
        let queued_failure = self.reject_queued(&mut order_receiver_channel).await.err();

        if let Err(error) = stream.close(None).await {
            error!(client = %self.client_id, instrument = %self.instrument, %error, "Failed to close orderbook connection");
        }

        if token.is_cancelled() {
            return Ok(());
        }

        if let Some(error) = queued_failure {
            return Err(error);
        }

        let Some(error) = session_error else {
            return Ok(());
        };

        Err(error)
    }

    async fn send_request(
        &self,
        stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
        request: &RequestMetadata,
    ) -> Result<()> {
        let message = match request.kind {
            RequestKind::Place => Request::place(request.order.clone()),
            RequestKind::Cancel => Request::cancel(
                request.order.order_id,
                request.order.price,
                request.order.side,
            ),
        };

        let payload = match serde_json::to_string(&message) {
            Ok(payload) => payload,
            Err(error) => {
                error!(client = %self.client_id, instrument = %self.instrument, %error, "Failed to serialize message");
                if let Err(error) = self.report_submission(request, false).await {
                    return Err(error);
                }
                return Ok(());
            }
        };

        match timeout(
            OUTBOUND_SEND_TIMEOUT,
            stream.send(Message::Text(payload.into())),
        )
        .await
        {
            Ok(Ok(())) => {
                if let Err(error) = self.report_submission(request, true).await {
                    return Err(error);
                }
                self.log_request(request);
                Ok(())
            }
            Ok(Err(error)) => {
                error!(client = %self.client_id, instrument = %self.instrument, %error, "Failed to send message to orderbook");
                if let Err(error) = self.report_submission(request, false).await {
                    return Err(error);
                }
                Err(eyre!(error))
            }
            Err(_) => {
                warn!(client = %self.client_id, instrument = %self.instrument, "Orderbook send timed out");
                if let Err(error) = self.report_submission(request, false).await {
                    return Err(error);
                }
                Err(eyre!("Orderbook send timed out"))
            }
        }
    }

    fn log_request(&self, request: &RequestMetadata) {
        match request.kind {
            RequestKind::Place => {
                info!(
                    client = %self.client_id,
                    instrument = %self.instrument,
                    price = %request.order.price,
                    size = %request.order.size,
                    side = %request.order.side,
                    "Order sent to orderbook"
                );
            }
            RequestKind::Cancel => {
                info!(
                    client = %self.client_id,
                    instrument = %self.instrument,
                    order = %request.order.order_id,
                    "Cancel sent to orderbook"
                );
            }
        }
    }

    async fn login(
        &self,
        stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
        last_exec_seq: u64,
        token: &CancellationToken,
    ) -> Result<()> {
        let login = LoginRequest::new(self.client_id, last_exec_seq);
        let payload = serde_json::to_string(&login).map_err(|error| eyre!(error))?;
        stream
            .send(Message::Text(payload.into()))
            .await
            .map_err(|error| eyre!(error))?;

        loop {
            select! {
                _ = token.cancelled() => {
                    return Err(eyre!("Cancelled while waiting for orderbook login response"));
                }
                message = stream.next() => {
                    let Some(message) = message else {
                        return Err(eyre!("Orderbook closed connection during login"));
                    };

                    let message = message.map_err(|error| eyre!(error))?;
                    let Message::Text(payload) = message else {
                        continue;
                    };

                    // TODO: loop until receiving expected message, but may block - add timeout?
                    let response =
                        serde_json::from_str::<LoginResponse>(&payload).map_err(|error| eyre!(error))?;
                    match response {
                        LoginResponse::LoginAccepted => {
                            info!(client = %self.client_id, instrument = %self.instrument, "Orderbook login accepted");
                            return Ok(());
                        }
                        LoginResponse::LoginRejected(rejection) => {
                            return Err(eyre!("Orderbook login rejected: {:?}", rejection.reason));
                        }
                    }
                }
            }
        }
    }

    async fn report_submission(&self, request: &RequestMetadata, submitted: bool) -> Result<()> {
        let RequestKind::Place = request.kind else {
            return Ok(());
        };

        let outcome = SubmitOutcome {
            order: request.order.clone(),
            submitted,
        };

        self.submit_outcome_channel
            .send(outcome)
            .await
            .map_err(|error| eyre!(error))
    }

    async fn clear_route(&self, order_sender_channel: &Sender<RequestMetadata>) {
        self.orderbook_registry
            .remove(&self.instrument, order_sender_channel)
            .await;
    }

    async fn reject_queued(
        &self,
        order_receiver_channel: &mut Receiver<RequestMetadata>,
    ) -> Result<()> {
        while let Ok(request) = order_receiver_channel.try_recv() {
            if let Err(error) = self.report_submission(&request, false).await {
                return Err(error);
            }
        }
        Ok(())
    }
}
