use crate::{
    metrics::FeedMetrics,
    proto::PriceUpdate,
    simulation::{manager::PriceManager, price::Price},
};
use eyre::Result;
use std::time::Duration;
use tokio::{select, sync::mpsc::Sender, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub struct Feed {
    /// Responsible for creating a new price value
    price_manager: PriceManager,
    /// Channel used to send generated prices to the publisher
    price_sender_channel: Sender<PriceUpdate>,
    /// Value to sleep between generations to provide a consistent rhythm
    publish_interval: u64,
    /// Tracks metrics regarding price generation
    metrics: FeedMetrics,
}

impl Feed {
    pub fn new(
        price_manager: PriceManager,
        price_sender_channel: Sender<PriceUpdate>,
        publish_interval: u64,
    ) -> Self {
        Self {
            price_manager,
            price_sender_channel,
            publish_interval,
            metrics: FeedMetrics::new(),
        }
    }

    pub async fn run(self, token: CancellationToken) -> Result<()> {
        let mut price_manager = self.price_manager;

        loop {
            let Price { instrument, value } = price_manager.next_price();

            match self
                .price_sender_channel
                .send(PriceUpdate {
                    instrument: instrument.to_string(),
                    value,
                })
                .await
            {
                Ok(()) => {
                    self.metrics.record_sent_price(&instrument, value);
                    info!(%instrument, price = value, "Sent price");
                }
                Err(error) => {
                    error!(%error, "Failed to send price");
                    return Err(error.into());
                }
            }

            select! {
                biased;

                // We requested a shutdown
                _ = token.cancelled() => break,

                // Slow down price generation to an interval
                _ = sleep(Duration::from_millis(self.publish_interval)) => {}
            }
        }

        Ok(())
    }
}
