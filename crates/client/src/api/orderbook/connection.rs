//! The connection is a specific instance of a link between a client and some external instrument/orderbook.
//!
//! It handles sending requests to its connected orderbook and forwards responses back into the engine

use crate::{
    api::{
        Response, WsUrl,
        orderbook::{ActiveOrderbooks, LoginRequest, LoginResponse, Request},
    },
    trade::Instrument,
};
use eyre::{Result, eyre};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::{
    net::TcpStream,
    select,
    sync::mpsc::{Sender, channel, error::TrySendError},
    time::timeout,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Quantity of time before a connection is dropped
const OUTBOUND_SEND_TIMEOUT: Duration = Duration::from_secs(2);
/// Number of orders waiting to be sent to the orderbook
const ORDER_QUEUE: usize = 128;

pub struct Connection {
    /// Random client ID
    client_id: Uuid,
    /// Used for logging and placing orders
    instrument: Instrument,
    /// Connect to the orderbook via websocket
    url: WsUrl,
    /// Receives orderbook responses and send back to engine
    response_sender_channel: Sender<Response>,
    /// Routes orders to logged-in orderbook connections
    active_orderbooks: ActiveOrderbooks,
    /// Cancels this connection
    token: CancellationToken,
}

impl Connection {
    pub fn new(
        client_id: Uuid,
        instrument: Instrument,
        url: WsUrl,
        response_sender_channel: Sender<Response>,
        active_orderbooks: ActiveOrderbooks,
        token: CancellationToken,
    ) -> Self {
        Self {
            client_id,
            instrument,
            url,
            response_sender_channel,
            active_orderbooks,
            token,
        }
    }

    pub async fn run(self) -> Result<()> {
        let (mut stream, _response) = connect_async(self.url.as_str()).await?;

        info!(
            client = %self.client_id,
            instrument = %self.instrument,
            url = %self.url,
            "Connected to orderbook"
        );

        // TODO: messy before&after login handling, clean up in proper issue
        if let Err(error) = login(&mut stream, self.client_id, &self.instrument, &self.token).await
        {
            if let Err(error) = stream.close(None).await {
                error!(client = %self.client_id, instrument = %self.instrument, %error, "Failed to close orderbook connection");
            }
            return Err(error);
        }

        // The engine sends requests to our central Orderbook type which looks up the orderbook service Connection by instrument.
        // It uses the sender to forward the request into the receiver which sends the request to the orderbook service.
        let (order_sender_channel, mut order_receiver_channel) = channel(ORDER_QUEUE);

        self.active_orderbooks
            .write()
            .await
            .insert(self.instrument.clone(), order_sender_channel.clone());

        loop {
            select! {
                biased;

                _ = self.token.cancelled() => break,

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

                            match self.response_sender_channel.try_send(response) {
                                Ok(()) => {}
                                Err(TrySendError::Closed(_)) => {
                                    error!(client = %self.client_id, instrument = %self.instrument, "Engine response channel closed");
                                    break;
                                }
                                Err(TrySendError::Full(_)) => {
                                    warn!(client = %self.client_id, instrument = %self.instrument, "Engine response queue full");
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            error!(client = %self.client_id, instrument = %self.instrument, "Orderbook service explicitly closed connection");
                            break;
                        }
                        Some(Err(error)) => {
                            error!(client = %self.client_id, instrument = %self.instrument, %error, "Unknown error");
                            break;
                        }
                        None => {
                            error!(client = %self.client_id, instrument = %self.instrument, "Disconnected from orderbook service");
                            break;
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
                        break;
                    };

                    let payload = match serde_json::to_string(&request.message) {
                        Ok(payload) => payload,
                        Err(error) => {
                            error!(client = %self.client_id, instrument = %self.instrument, %error, "Failed to serialize message");
                            continue;
                        }
                    };

                    match timeout(
                        OUTBOUND_SEND_TIMEOUT,
                        stream.send(Message::Text(payload.into())),
                    )
                    .await
                    {
                        Ok(Ok(())) => {
                            match &request.message {
                                Request::Place { price, size, side, .. } => {
                                    info!(
                                        client = %self.client_id,
                                        instrument = %self.instrument,
                                        %price,
                                        %size,
                                        %side,
                                        "Order sent to orderbook"
                                    );
                                }
                                Request::Cancel { order_id, .. } => {
                                    info!(
                                        client = %self.client_id,
                                        instrument = %self.instrument,
                                        order = %order_id,
                                        "Cancel sent to orderbook"
                                    );
                                }
                            }
                        }
                        Ok(Err(error)) => {
                            error!(client = %self.client_id, instrument = %self.instrument, %error, "Failed to send message to orderbook");
                            break;
                        }
                        Err(_) => {
                            warn!(client = %self.client_id, instrument = %self.instrument, "Orderbook send timed out");
                            break;
                        }
                    }
                }
            }
        }

        let mut routes = self.active_orderbooks.write().await;
        if routes
            .get(&self.instrument)
            .is_some_and(|registered| registered.same_channel(&order_sender_channel))
        {
            routes.remove(&self.instrument);
        }
        drop(routes);

        if let Err(error) = stream.close(None).await {
            error!(client = %self.client_id, instrument = %self.instrument, %error, "Failed to close orderbook connection");
        }

        Ok(())
    }
}

async fn login(
    stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    client_id: Uuid,
    instrument: &Instrument,
    token: &CancellationToken,
) -> Result<()> {
    let login = LoginRequest::new(client_id);
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
                        info!(client = %client_id, instrument = %instrument, "Orderbook login accepted");
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
