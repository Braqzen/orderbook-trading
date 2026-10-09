//! The market data provider subscribes and listens to price updates from specific instruments then forwards updates into the decision engine
//!
//! The client subscribes to specific instruments and filters out other updates because the provider sends all instrument updates through 1 connection.
//! If the client is still connected to the orderbook/instrument the provider forwards the update to the engine for trade decisions.

use crate::{
    api::{
        WsUrl,
        market::{
            MarketPrice,
            request::{ClientRequest, Operation},
            response::{PriceUpdate, Response, SubscriptionResponse},
            subscription::Subscriptions,
        },
        orderbook::ActiveOrderbooks,
    },
    metrics::ClientMetrics,
    trade::{Instrument, Price},
};
use eyre::Result;
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::{
    select,
    sync::mpsc::{Sender, error::TrySendError},
    time::{Instant, sleep_until},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Minimum time bound for when a subscription update may occur
const MIN_ACTION_DELAY_SECONDS: u64 = 60;
/// Maximum time bound for when a subscription update may occur
const MAX_ACTION_DELAY_SECONDS: u64 = 180;

pub struct MarketDataProvider {
    /// Used for attaching to logs
    client_id: Uuid,
    /// Connect to the market data provider to receive price updates
    url: WsUrl,
    /// Original and current instrument subscription state
    subscriptions: Subscriptions,
    /// Channel used to send price events to the engine
    price_sender_channel: Sender<MarketPrice>,
    /// Orderbooks that are currently logged in
    active_orderbooks: ActiveOrderbooks,
    /// Track metrics
    metrics: ClientMetrics,
}

impl MarketDataProvider {
    pub fn new(
        client_id: Uuid,
        url: WsUrl,
        instruments: Vec<Instrument>,
        price_sender_channel: Sender<MarketPrice>,
        active_orderbooks: ActiveOrderbooks,
        metrics: ClientMetrics,
    ) -> Self {
        Self {
            client_id,
            url,
            subscriptions: Subscriptions::new(instruments),
            price_sender_channel,
            active_orderbooks,
            metrics,
        }
    }

    pub async fn run(mut self, token: CancellationToken) -> Result<()> {
        // Create a single ws connection to the market provider which sends all instruments through 1 connection
        let (mut stream, _response) = connect_async(self.url.as_str()).await?;

        // First msg to provider tells them which instruments client is interested in for updates
        let available_instruments = self.subscriptions.available();
        let subscribe = ClientRequest::subscribe(available_instruments.clone());

        let payload = serde_json::to_string(&subscribe)?;
        if let Err(error) = stream.send(Message::Text(payload.into())).await {
            error!(client = %self.client_id, %error, "Failed to send initial subscription");
            return Err(error.into());
        }

        info!(client = %self.client_id, instruments = ?available_instruments, "Subscribed to market data provider");
        for instrument in self.subscriptions.subscribed() {
            self.metrics.record_subscription(instrument, true);
        }

        let mut subscription_time = subscription_tick();

        loop {
            select! {
                _ = token.cancelled() => break,

                // Listen to market data provider events and forward to engine for next client step
                market_message = stream.next() => {
                    match market_message {
                        Some(Ok(Message::Text(text))) => {
                            let response = match serde_json::from_str::<Response>(text.as_str()) {
                                Ok(response) => response,
                                Err(error) => {
                                    warn!(client = %self.client_id, %error, %text, "Failed to parse market data provider message");
                                    continue;
                                }
                            };

                            match response {
                                Response::Price(price) => {
                                    if self.handle_price_update(price).await
                                        == PriceUpdateResult::EngineChannelClosed
                                    {
                                        // Cannot send prices to engine anymore, stop everything
                                        break;
                                    }
                                }
                                Response::Rejected => {
                                    warn!(client = %self.client_id, "Market data provider rejected request");
                                }
                                Response::Subscription(subscription) => match subscription {
                                    SubscriptionResponse::Subscribed(instruments) => {
                                        info!(client = %self.client_id, ?instruments, "Market data subscription accepted");
                                    }
                                    SubscriptionResponse::PartiallySubscribed { subscribed, rejected } => {
                                        warn!(client = %self.client_id, ?subscribed, ?rejected, "Market data subscription partially accepted");
                                    }
                                    SubscriptionResponse::Unsubscribed(instruments) => {
                                        info!(client = %self.client_id, ?instruments, "Market data unsubscribe accepted");
                                    }
                                    SubscriptionResponse::PartiallyUnsubscribed { unsubscribed, rejected } => {
                                        warn!(client = %self.client_id, ?unsubscribed, ?rejected, "Market data unsubscribe partially accepted");
                                    }
                                    SubscriptionResponse::Rejected(instruments) => {
                                        warn!(client = %self.client_id, ?instruments, "Market data provider rejected subscription change");
                                    }
                                },
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

                _ = sleep_until(subscription_time) => {
                    let (operation, instruments) = match self.subscriptions.random_action() {
                        Some(action) => action,
                        None => {
                            subscription_time = subscription_tick();
                            continue;
                        }
                    };


                    let request = match operation {
                        Operation::Subscribe => ClientRequest::subscribe(instruments.clone()),
                        Operation::Unsubscribe => ClientRequest::unsubscribe(instruments.clone()),
                    };

                    let payload = match serde_json::to_string(&request) {
                        Ok(payload) => payload,
                        Err(error) => {
                            error!(client = %self.client_id, %error, "Failed to serialize subscription change");
                            break;
                        }
                    };

                    // Causes incoming provider messages to pause/queue until this write flushes
                    if let Err(error) = stream.send(Message::Text(payload.into())).await {
                        error!(client = %self.client_id, %error, "Failed to send subscription change");
                        break;
                    }

                    self.subscriptions.update(operation, &instruments);

                    for (instrument, subscribed) in self.subscriptions.states() {
                        self.metrics.record_subscription(instrument, subscribed);
                    }

                    info!(
                        client = %self.client_id,
                        %operation,
                        "Sent market data subscription change"
                    );

                    subscription_time = subscription_tick();
                }
            }
        }

        // Reset values to 0 otherwise metrics retain previous value
        for instrument in self.subscriptions.subscribed() {
            self.metrics.record_subscription(instrument, false);
        }

        if let Err(error) = stream.close(None).await {
            error!(client = %self.client_id, %error, "Failed to close market data provider connection");
        }

        Ok(())
    }

    async fn handle_price_update(&self, price: PriceUpdate) -> PriceUpdateResult {
        let instrument = match Instrument::try_from(price.instrument.as_str()) {
            Ok(instrument) => instrument,
            Err(error) => {
                warn!(client = %self.client_id, %error, "Received invalid instrument");
                return PriceUpdateResult::Continue;
            }
        };

        // Do not forward price updates deeper into the engine if client is not connected to orderbook
        if !self
            .active_orderbooks
            .read()
            .await
            .contains_key(&instrument)
        {
            return PriceUpdateResult::Continue;
        }

        let value = match Price::try_from(price.value) {
            Ok(value) => value,
            Err(error) => {
                warn!(client = %self.client_id, %error, "Received invalid price");
                return PriceUpdateResult::Continue;
            }
        };

        // Separate transport type from internal logical type; increase type safety
        let price = MarketPrice::new(instrument, value);

        info!(client = %self.client_id, instrument = %price.instrument, price = %price.value, "Price update");

        match self.price_sender_channel.try_send(price) {
            Ok(()) => PriceUpdateResult::Continue,
            Err(TrySendError::Full(_)) => {
                warn!(client = %self.client_id, "Engine price queue full");
                PriceUpdateResult::Continue
            }
            Err(TrySendError::Closed(_)) => {
                error!(client = %self.client_id, "Engine price channel closed");
                PriceUpdateResult::EngineChannelClosed
            }
        }
    }
}

#[derive(PartialEq, Eq)]
enum PriceUpdateResult {
    Continue,
    EngineChannelClosed,
}

/// Randomly select a time to potentially update instrument subscriptions
fn subscription_tick() -> Instant {
    Instant::now()
        + Duration::from_secs(rand::random_range(
            MIN_ACTION_DELAY_SECONDS..=MAX_ACTION_DELAY_SECONDS,
        ))
}
