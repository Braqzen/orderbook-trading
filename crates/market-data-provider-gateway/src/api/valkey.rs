//! Subscribes to the named market-data price channel and publishes each price on that instrument's channel.
//!
//! The published message is the instrument which is subsequently queried with GET to retrieve the price.
//! A missing key means the price has expired.

use crate::api::price::{PriceChannels, PriceUpdate};
use eyre::Result;
use futures_util::StreamExt;
use redis::{AsyncCommands, Client};
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Valkey name of the price cache. An instrument price is stored at `{PRICE_CACHE_KEY}:{instrument}` and that instrument is published on this name.
const PRICE_CACHE_KEY: &str = "market-data-provider";

pub struct PriceReader {
    /// Valkey connection URL
    url: String,
    /// Instrument-specific price channels
    prices: PriceChannels,
}

impl PriceReader {
    pub fn new(url: String, prices: PriceChannels) -> Self {
        Self { url, prices }
    }

    pub async fn run(self, token: CancellationToken) -> Result<()> {
        let client = Client::open(self.url.as_str())?;
        let mut cache = client.get_connection_manager().await?;
        let mut pubsub = client.get_async_pubsub().await?;

        pubsub.subscribe(PRICE_CACHE_KEY).await?;

        info!(
            price_cache_key = PRICE_CACHE_KEY,
            "Subscribed to Valkey price events"
        );

        // Stream for pubsub which receives messages containing the instrument (not the price)
        let mut messages = pubsub.on_message();

        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                // Provider published an update to an instrument
                message = messages.next() => {
                    let Some(message) = message else {
                        return Err(std::io::Error::other("Valkey price subscription closed").into());
                    };

                    // The message should be a string containing the instrument
                    let instrument = match message.get_payload::<String>() {
                        Ok(instrument) => instrument,
                        Err(error) => {
                            error!(%error, "Failed to read price event");
                            continue;
                        }
                    };

                    // Storage format for of the key to get the value of the published instrument
                    let key = format!("{PRICE_CACHE_KEY}:{instrument}");

                    let value: Option<f64> = match cache.get(&key).await {
                        Ok(value) => value,
                        Err(error) if error.is_unrecoverable_error() => match cache.get(&key).await {
                            Ok(value) => value,
                            Err(error) => {
                                error!(%error, instrument, "Failed to read price");
                                continue;
                            }
                        },
                        Err(error) => {
                            error!(%error, instrument, "Failed to read price");
                            continue;
                        }
                    };

                    let Some(value) = value else {
                        warn!(instrument, "Price missing");
                        continue;
                    };

                    info!(instrument, price = value, "Price update");

                    self.prices
                        .publish(PriceUpdate { instrument, value })
                        .await;
                }
            }
        }

        Ok(())
    }
}
