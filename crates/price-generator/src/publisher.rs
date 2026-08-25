use crate::proto::{PriceUpdate, market_data_provider_client::MarketDataProviderClient};
use eyre::Result;
use tokio::{select, sync::mpsc::Receiver};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

pub struct Publisher {
    market_data_provider_url: String,
    price_receiver_channel: Receiver<PriceUpdate>,
}

impl Publisher {
    pub fn new(
        market_data_provider_url: String,
        price_receiver_channel: Receiver<PriceUpdate>,
    ) -> Self {
        Self {
            market_data_provider_url,
            price_receiver_channel,
        }
    }

    pub async fn run(self, token: CancellationToken) -> Result<()> {
        let mut client = MarketDataProviderClient::connect(self.market_data_provider_url).await?;
        let publish = client.publish_price(ReceiverStream::new(self.price_receiver_channel));
        tokio::pin!(publish);

        select! {
            biased;

            _ = token.cancelled() => Ok(()),

            result = publish => match result {
                Ok(_) => Err(std::io::Error::other("price stream closed").into()),
                Err(error) => Err(error.into()),
            },
        }
    }
}
