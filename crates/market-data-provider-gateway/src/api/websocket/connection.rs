//! Handles one client WebSocket connection.
//!
//! Subscribe adds instrument price receivers. Unsubscribe removes them. This connection therefore
//! receives price updates only for instruments the client currently selected.

use crate::{
    api::{
        price::PriceChannels,
        websocket::{
            request::{ClientRequest, Instruction, Operation},
            response::Response,
            subscription::Subscriptions,
        },
    },
    metrics::GatewayMetrics,
};
use eyre::Result;
use futures_util::SinkExt;
use std::net::SocketAddr;
use tokio::{net::TcpStream, select};
use tokio_stream::{StreamExt, wrappers::errors::BroadcastStreamRecvError};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub struct Connection {
    /// Stream to receive messages through
    stream: TcpStream,
    /// Address to identify the client
    client: SocketAddr,
    /// Instrument-specific price channels
    prices: PriceChannels,
    /// Token to handle shutdown requests
    token: CancellationToken,
    /// Tracks metrics
    metrics: GatewayMetrics,
}

impl Connection {
    pub fn new(
        stream: TcpStream,
        client: SocketAddr,
        prices: PriceChannels,
        token: CancellationToken,
        metrics: GatewayMetrics,
    ) -> Self {
        Self {
            stream,
            client,
            prices,
            token,
            metrics,
        }
    }

    pub async fn run(self) -> Result<()> {
        // Keep one stream because splitting does not guarantee delivery; reliable replay requires persistent message IDs
        let mut ws_stream = match accept_async(self.stream).await {
            Ok(stream) => stream,
            Err(error) => {
                error!(%error, "WebSocket handshake failed");
                return Ok(());
            }
        };

        self.metrics.client_connected();

        // Track which instruments/channel receivers the client has subscribed to
        // All handles are managed by this single type, query 1 type instead of multiple channels
        let mut subscriptions = Subscriptions::new();

        loop {
            select! {
                biased;

                _ = self.token.cancelled() => break,

                // Listen for messages sent by the client
                message = ws_stream.next() => {
                    match message {
                        Some(Ok(Message::Text(payload))) => {
                            let request = match serde_json::from_str::<ClientRequest>(&payload) {
                                Ok(request) => request,
                                Err(error) => {
                                    warn!(client = %self.client, %error, "Received invalid client request");
                                    if send_response(&mut ws_stream, &Response::Rejected).await
                                        == WriteResult::Failed
                                    {
                                        break;
                                    }
                                    continue;
                                }
                            };

                            let Instruction::Instruments { instruments } = request.instruction;
                            let response = if instruments.is_empty() {
                                warn!(client = %self.client, operation = %request.op, "Rejected empty subscription request");
                                Response::Rejected
                            } else {
                                let (accepted, rejected) = subscriptions
                                    .update(&self.prices, request.op, &instruments)
                                    .await;

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

                            if send_response(&mut ws_stream, &response).await == WriteResult::Failed
                            {
                                break;
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Err(error)) => {
                            error!(client = %self.client, %error, "WebSocket connection failed");
                            break;
                        }
                        _ => {}
                    }
                }

                // A price update has been sent for 1 of the subscribed instruments
                price = subscriptions.next(), if !subscriptions.is_empty() => {
                    match price {
                        Some((_, Ok(price))) => {
                            info!(
                                client = %self.client,
                                instrument = price.instrument,
                                price = price.value,
                                "Sending price"
                            );

                            if send_response(&mut ws_stream, &Response::price(price)).await
                                == WriteResult::Failed
                            {
                                break;
                            }
                        }
                        Some((instrument, Err(BroadcastStreamRecvError::Lagged(skipped)))) => {
                            warn!(client = %self.client, instrument, skipped, "WebSocket client lagged");
                        }
                        None => break,
                    }
                }
            }
        }

        for instrument in subscriptions.subscriptions() {
            self.metrics.instrument_unsubscribed(instrument);
        }
        self.metrics.client_disconnected();

        if let Err(error) = ws_stream.close(None).await {
            error!(client = %self.client, %error, "Failed to close WebSocket connection");
        }

        Ok(())
    }
}

#[derive(PartialEq, Eq)]
enum WriteResult {
    Sent,
    Failed,
}

/// Forwards the subscribed price update to the connected client
async fn send_response(
    stream: &mut tokio_tungstenite::WebSocketStream<TcpStream>,
    response: &Response,
) -> WriteResult {
    let payload = match serde_json::to_string(response) {
        Ok(payload) => payload,
        Err(error) => {
            error!(%error, "Failed to serialize client response");
            return WriteResult::Failed;
        }
    };

    if let Err(error) = stream.send(Message::Text(payload.into())).await {
        error!(%error, "Failed to send client response");
        return WriteResult::Failed;
    }

    WriteResult::Sent
}
