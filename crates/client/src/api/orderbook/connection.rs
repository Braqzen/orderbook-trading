use crate::{
    api::{
        Response, WsUrl,
        orderbook::{ActiveOrderbooks, LoginRequest, LoginResponse, Request, RequestMetadata},
    },
    trade::Instrument,
};
use eyre::{Result, eyre};
use futures_util::{SinkExt, StreamExt};
use tokio::{
    net::TcpStream,
    select,
    sync::mpsc::{Receiver, Sender, error::TrySendError},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct Connection {
    /// Random client ID
    client_id: Uuid,
    /// Used for logging and placing orders
    instrument: Instrument,
    /// Connect to the orderbook via websocket
    url: WsUrl,
    order_sender_channel: Sender<RequestMetadata>,
    /// Receives orders and forwards to orderbook
    order_receiver_channel: Receiver<RequestMetadata>,
    /// Receives orderbook responses and send back to engine
    response_sender_channel: Sender<Response>,
    /// Routes orders to logged-in orderbook connections
    active_orderbooks: ActiveOrderbooks,
}

impl Connection {
    pub fn new(
        client_id: Uuid,
        instrument: Instrument,
        url: WsUrl,
        order_sender_channel: Sender<RequestMetadata>,
        order_receiver_channel: Receiver<RequestMetadata>,
        response_sender_channel: Sender<Response>,
        active_orderbooks: ActiveOrderbooks,
    ) -> Self {
        Self {
            client_id,
            instrument,
            url,
            order_sender_channel,
            order_receiver_channel,
            response_sender_channel,
            active_orderbooks,
        }
    }

    pub async fn run(mut self, token: CancellationToken) -> Result<()> {
        let (mut stream, _response) = connect_async(self.url.as_str()).await?;

        info!(
            client = %self.client_id,
            instrument = %self.instrument,
            url = %self.url,
            "Connected to orderbook"
        );

        // TODO: messy before&after login handling, clean up in proper issue
        if let Err(error) = login(&mut stream, self.client_id, &self.instrument, &token).await {
            if let Err(error) = stream.close(None).await {
                error!(client = %self.client_id, instrument = %self.instrument, %error, "Failed to close orderbook connection");
            }
            return Err(error);
        }

        self.active_orderbooks
            .write()
            .await
            .insert(self.instrument.clone(), self.order_sender_channel.clone());

        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                // Orderbook has responded, forward to engine
                message = stream.next() => {
                    match message {
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

                // Received request to forward to orderbook
                request = self.order_receiver_channel.recv() => {
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

                    match stream.send(Message::Text(payload.into())).await {
                        Ok(()) => {
                            match &request.message {
                                Request::Place { price, size, side, .. } => {
                                    info!(
                                        client = %self.client_id,
                                        instrument = %self.instrument,
                                        price = %price,
                                        size = %size,
                                        side = %side,
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
                        },
                        Err(error) => {
                            error!(client = %self.client_id, instrument = %self.instrument, %error, "Failed to send message to orderbook");
                            break;
                        }
                    }
                }
            }
        }

        let mut routes = self.active_orderbooks.write().await;
        if routes
            .get(&self.instrument)
            .is_some_and(|registered| registered.same_channel(&self.order_sender_channel))
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
