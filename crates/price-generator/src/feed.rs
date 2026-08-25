use crate::{
    metrics::FeedMetrics,
    price::{Price, PriceManager},
    proto::PriceUpdate,
};
use eyre::Result;
use std::time::Duration;
use tokio::{select, sync::mpsc::Sender, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub struct Feed {
    price_manager: PriceManager,
    price_sender_channel: Sender<PriceUpdate>,
    publish_interval: u64,
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
                    break;
                }
            }

            select! {
                biased;

                _ = token.cancelled() => break,
                _ = sleep(Duration::from_millis(self.publish_interval)) => {}
            }
        }

        Ok(())
    }
}
