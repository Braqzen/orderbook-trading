use crate::proto::{PriceUpdate, price_aggregator_client::PriceAggregatorClient};
use eyre::Result;
use tokio::{select, sync::mpsc::Receiver};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

pub struct Publisher {
    /// Where to send gRPC requests to
    price_aggregator_url: String,
    /// Channel to receive price updates from [`Feed`]s
    price_receiver_channel: Receiver<PriceUpdate>,
}

impl Publisher {
    pub fn new(
        price_aggregator_url: String,
        price_receiver_channel: Receiver<PriceUpdate>,
    ) -> Self {
        Self {
            price_aggregator_url,
            price_receiver_channel,
        }
    }

    pub async fn run(self, token: CancellationToken) -> Result<()> {
        // Establish a connection to the Price Aggregator service
        let mut client = PriceAggregatorClient::connect(self.price_aggregator_url).await?;
        let publish = client.publish_price(ReceiverStream::new(self.price_receiver_channel));

        select! {
            biased;

            // We requested a shutdown
            _ = token.cancelled() => Ok(()),

            // Something ended the gRPC stream unexpectedly
            result = publish => match result {
                Ok(_) => Err(std::io::Error::other("price stream closed").into()),
                Err(error) => Err(error.into()),
            },
        }
    }
}
