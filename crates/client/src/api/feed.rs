use crate::{api::MarketPrice, trade::Instrument};
use eyre::Result;
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::{select, sync::mpsc::Sender};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub struct MarketFeed {
    url: String,
    price_sender_channel: Sender<MarketPrice>,
}

impl MarketFeed {
    pub fn new(url: String, price_sender_channel: Sender<MarketPrice>) -> Self {
        Self {
            url,
            price_sender_channel,
        }
    }

    pub async fn run(self, token: CancellationToken) -> Result<()> {
        let (mut stream, _response) = connect_async(self.url.clone()).await?;

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
                                    warn!(%error, %text, "Failed to parse price update");
                                    continue;
                                }
                            };
                            let instrument = match Instrument::try_from(price.instrument.as_str()) {
                                Ok(instrument) => instrument,
                                Err(error) => {
                                    warn!(%error, "Received invalid instrument");
                                    continue;
                                }
                            };
                            let price = MarketPrice::new(instrument, price.value);

                            info!(instrument = %price.instrument, price = price.value, "Price update");

                            if self.price_sender_channel.send(price).await.is_err() {
                                error!("Failed to send price through channel");
                                break;
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            error!("Market service explicitly closed connection");
                            break;
                        }
                        Some(Err(error)) => {
                            error!(%error, "Unknown error");
                            break;
                        }
                        None => {
                            error!("Disconnected from market service");
                            break;
                        }
                        _ => {
                            warn!("Skipping unexpected message");
                        }
                    }
                }
            }
        }

        if let Err(error) = stream.close(None).await {
            error!(%error, "Failed to close market feed connection");
        }

        Ok(())
    }
}

#[derive(Deserialize)]
struct PriceUpdate {
    instrument: String,
    value: f64,
}
