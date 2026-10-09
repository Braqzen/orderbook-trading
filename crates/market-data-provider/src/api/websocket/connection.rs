//! Handles one client WebSocket connection
//!
//! Receives subscribe and unsubscribe requests, then forwards price updates for the instruments that client has subscribed to.

use crate::{
    api::websocket::{
        request::{ClientRequest, Instruction, Operation},
        response::Response,
        subscription::Subscriptions,
    },
    metrics::MarketDataProviderMetrics,
    proto::PriceUpdate,
};
use eyre::Result;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::{
    net::TcpStream,
    select,
    sync::broadcast::{Receiver, error::RecvError},
};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub struct Connection {
    /// Stream to receive messages through
    stream: TcpStream,
    /// Address to identify the client
    client: SocketAddr,
    /// Individual subscribed receiver per client
    price_receiver_channel: Receiver<PriceUpdate>,
    /// Token to handle shutdown requests
    token: CancellationToken,
    /// Tracks metrics
    metrics: MarketDataProviderMetrics,
}

impl Connection {
    pub fn new(
        stream: TcpStream,
        client: SocketAddr,
        price_receiver_channel: Receiver<PriceUpdate>,
        token: CancellationToken,
        metrics: MarketDataProviderMetrics,
    ) -> Self {
        Self {
            stream,
            client,
            price_receiver_channel,
            token,
            metrics,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        // Keep one stream because splitting does not guarantee delivery; reliable replay requires persistent message IDs
        let mut ws_stream = match accept_async(self.stream).await {
            Ok(stream) => stream,
            Err(error) => {
                error!(%error, "WebSocket handshake failed");
                return Ok(());
            }
        };

        self.metrics.client_connected();

        // Track which instruments the client has subscribed to and only send those updates
        let mut subscriptions = Subscriptions::new();

        loop {
            select! {
                biased;

                _ = self.token.cancelled() => break,

                // Listen for messages send by the client to the server connection
                message = ws_stream.next() => {
                    match message {
                        Some(Ok(Message::Text(payload))) => {
                            let request = match serde_json::from_str::<ClientRequest>(&payload) {
                                Ok(request) => request,
                                Err(error) => {
                                    // Respond with a protocol error instead of silently ignoring invalid requests
                                    warn!(%error, "Received invalid client request");

                                    let payload = match serde_json::to_string(&Response::Rejected) {
                                        Ok(payload) => payload,
                                        Err(error) => {
                                            error!(%error, "Failed to serialize invalid request response");
                                            break;
                                        }
                                    };

                                    if let Err(error) =
                                        ws_stream.send(Message::Text(payload.into())).await
                                    {
                                        error!(%error, "Failed to send invalid request response");
                                        break;
                                    }

                                    continue;
                                }
                            };

                            let Instruction::Instruments { instruments } = request.instruction;

                            // Bad request: client sent empty list
                            let response = if instruments.is_empty() {
                                warn!(client = %self.client, operation = %request.op, "Rejected empty subscription request");
                                Response::Rejected
                            } else {
                                let (accepted, rejected) =
                                    subscriptions.update(request.op, &instruments);

                                for instrument in &accepted {
                                    match request.op {
                                        Operation::Subscribe => {
                                            self.metrics.instrument_subscribed(instrument);
                                        }
                                        Operation::Unsubscribe => {
                                            self.metrics.instrument_unsubscribed(instrument);
                                        }
                                    }
                                }

                                if rejected.is_empty() {
                                    info!(
                                        client = %self.client,
                                        operation = %request.op,
                                        instruments = ?accepted,
                                        count = subscriptions.subscriptions().count(),
                                        "Client subscription changed"
                                    );
                                } else {
                                    warn!(
                                        client = %self.client,
                                        operation = %request.op,
                                        accepted = ?accepted,
                                        rejected = ?rejected,
                                        count = subscriptions.subscriptions().count(),
                                        "Client subscription partially changed"
                                    );
                                }

                                Response::subscription(request.op, accepted, rejected)
                            };

                            let payload = match serde_json::to_string(&response) {
                                Ok(payload) => payload,
                                Err(error) => {
                                    error!(%error, "Failed to serialize subscription response");
                                    break;
                                }
                            };

                            if let Err(error) =
                                ws_stream.send(Message::Text(payload.into())).await
                            {
                                error!(%error, "Failed to send subscription response");
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

                // Listen for price updates from gRPC
                price_update = self.price_receiver_channel.recv() => {
                    // TODO: it would be nice to track metrics for each client receiver queue
                    match price_update {
                        Ok(price_update) => {
                            // Only forwards subscribed instruments
                            if !subscriptions.subscribed(&price_update.instrument) {
                                continue;
                            }

                            info!(
                                client = %self.client,
                                instrument = price_update.instrument,
                                price = price_update.value,
                                "Sending price"
                            );

                            let response = Response::price(price_update);
                            let payload = match serde_json::to_string(&response) {
                                Ok(payload) => payload,
                                Err(error) => {
                                    error!(%error, "Failed to serialize price");
                                    break;
                                }
                            };

                            // Waits till we send then continue looping
                            // Only block this connection
                            if let Err(error) = ws_stream
                                .send(Message::Text(payload.into()))
                                .await
                            {
                                error!(%error, "Failed to send price");
                                break;
                            }
                        }
                        Err(RecvError::Lagged(skipped)) => {
                            warn!(skipped, "WebSocket client lagged");
                        }
                        Err(RecvError::Closed) => break,
                    }
                }
            }
        }

        for instrument in subscriptions.subscriptions() {
            self.metrics.instrument_unsubscribed(instrument);
        }
        self.metrics.client_disconnected();

        if let Err(error) = ws_stream.close(None).await {
            error!(%error, "Failed to close WebSocket connection");
        }

        Ok(())
    }
}
