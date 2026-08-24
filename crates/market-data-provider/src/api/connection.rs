use crate::{
    api::request::{ClientRequest, Instruction, Operation},
    metrics::MarketDataProviderMetrics,
    proto::PriceUpdate,
};
use eyre::Result;
use futures_util::{SinkExt, StreamExt};
use std::{collections::HashSet, net::SocketAddr};
use tokio::{
    net::TcpStream,
    select,
    sync::broadcast::{Receiver, error::RecvError},
};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub struct Connection {
    stream: TcpStream,
    client: SocketAddr,
    price_receiver_channel: Receiver<PriceUpdate>,
    token: CancellationToken,
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
        let mut ws_stream = match accept_async(self.stream).await {
            Ok(stream) => stream,
            Err(error) => {
                error!(%error, "WebSocket handshake failed");
                return Ok(());
            }
        };

        let mut subscriptions = HashSet::new();
        self.metrics.client_connected();

        loop {
            select! {
                biased;

                _ = self.token.cancelled() => break,

                message = ws_stream.next() => {
                    match message {
                        Some(Ok(Message::Text(payload))) => {
                            let request = match serde_json::from_str::<ClientRequest>(&payload) {
                                Ok(request) => request,
                                Err(error) => {
                                    warn!(%error, "Received invalid client request");
                                    continue;
                                }
                            };

                            match request.op {
                                Operation::Subscribe => {
                                    let Instruction::Instruments { instruments } =
                                        request.instruction;

                                    for instrument in instruments {
                                        if subscriptions.insert(instrument.clone()) {
                                            self.metrics.instrument_subscribed(&instrument);
                                        }
                                    }

                                    info!(
                                        client = %self.client,
                                        count = subscriptions.len(),
                                        "Client subscribed"
                                    );
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

                price_update = self.price_receiver_channel.recv() => {
                    match price_update {
                        Ok(price_update) => {
                            if !subscriptions.contains(&price_update.instrument) {
                                continue;
                            }

                            info!(
                                client = %self.client,
                                instrument = price_update.instrument,
                                price = price_update.value,
                                "Sending price"
                            );

                            let payload = serde_json::json!({
                                "instrument": price_update.instrument,
                                "value": price_update.value,
                            });

                            if let Err(error) = ws_stream
                                .send(Message::Text(payload.to_string().into()))
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

        for instrument in subscriptions {
            self.metrics.instrument_unsubscribed(&instrument);
        }
        self.metrics.client_disconnected();

        if let Err(error) = ws_stream.close(None).await {
            error!(%error, "Failed to close WebSocket connection");
        }

        Ok(())
    }
}
