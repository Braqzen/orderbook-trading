use crate::{
    api::WsUrl,
    api::market::{MarketPrice, request::ClientRequest},
    metrics::ClientMetrics,
    trade::{Instrument, Price},
};
use eyre::Result;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::{select, sync::mpsc::Sender};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct MarketDataProvider {
    /// Used for attaching to logs
    client_id: Uuid,
    /// Connect to the market data provider to receive price updates
    url: WsUrl,
    /// Set of instruments to subscribe to from the randomly generated state
    instruments: Vec<Instrument>,
    /// Channel used to send price events to the engine
    price_sender_channel: Sender<MarketPrice>,
    /// Track metrics
    metrics: ClientMetrics,
}

impl MarketDataProvider {
    pub fn new(
        client_id: Uuid,
        url: WsUrl,
        instruments: Vec<Instrument>,
        price_sender_channel: Sender<MarketPrice>,
        metrics: ClientMetrics,
    ) -> Self {
        Self {
            client_id,
            url,
            instruments,
            price_sender_channel,
            metrics,
        }
    }

    pub async fn run(self, token: CancellationToken) -> Result<()> {
        let (mut stream, _response) = connect_async(self.url.as_str()).await?;

        // TODO: ack/err handling instead of silently fail without response
        // First msg to provider tells them which instruments client is interested in for updates
        let subscribe = ClientRequest::subscribe(
            self.instruments
                .iter()
                .map(|instrument| instrument.to_string())
                .collect(),
        );
        let payload = serde_json::to_string(&subscribe)?;
        stream.send(Message::Text(payload.into())).await?;

        for instrument in &self.instruments {
            self.metrics.record_subscription(instrument, true);
        }

        info!(client = %self.client_id, instruments = ?self.instruments, "Subscribed to market data provider");

        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                // Listen to market data provider events and forward to engine for next step
                message = stream.next() => {
                    match message {
                        Some(Ok(Message::Text(text))) => {
                            let price = match serde_json::from_str::<PriceUpdate>(text.as_str()) {
                                Ok(price) => price,
                                Err(error) => {
                                    warn!(client = %self.client_id, %error, %text, "Failed to parse price update");
                                    continue;
                                }
                            };
                            let instrument = match Instrument::try_from(price.instrument.as_str()) {
                                Ok(instrument) => instrument,
                                Err(error) => {
                                    warn!(client = %self.client_id, %error, "Received invalid instrument");
                                    continue;
                                }
                            };
                            let value = match Price::try_from(price.value) {
                                Ok(value) => value,
                                Err(error) => {
                                    warn!(client = %self.client_id, %error, "Received invalid price");
                                    continue;
                                }
                            };

                            // Separate transport type from internal logical type; increase type safety
                            let price = MarketPrice::new(instrument, value);

                            info!(client = %self.client_id, instrument = %price.instrument, price = %price.value, "Price update");

                            if self.price_sender_channel.send(price).await.is_err() {
                                error!(client = %self.client_id, "Failed to send price through channel");
                                break;
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            error!(client = %self.client_id, "Market data provider explicitly closed connection");
                            break;
                        }
                        Some(Err(error)) => {
                            error!(client = %self.client_id, %error, "Unknown error");
                            break;
                        }
                        None => {
                            error!(client = %self.client_id, "Disconnected from market data provider");
                            break;
                        }
                        _ => {
                            warn!(client = %self.client_id, "Skipping unexpected message");
                        }
                    }
                }
            }
        }

        // Reset values to 0 otherwise metrics retain previous value
        for instrument in &self.instruments {
            self.metrics.record_subscription(instrument, false);
        }

        if let Err(error) = stream.close(None).await {
            error!(client = %self.client_id, %error, "Failed to close market data provider connection");
        }

        Ok(())
    }
}

#[derive(Deserialize)]
struct PriceUpdate {
    instrument: String,
    value: f64,
}
