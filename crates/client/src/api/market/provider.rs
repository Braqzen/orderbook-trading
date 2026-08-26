use crate::{
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
    client_id: Uuid,
    url: String,
    instruments: Vec<Instrument>,
    price_sender_channel: Sender<MarketPrice>,
    metrics: ClientMetrics,
}

impl MarketDataProvider {
    pub fn new(
        client_id: Uuid,
        url: String,
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
        let (mut stream, _response) = connect_async(self.url.clone()).await?;

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
