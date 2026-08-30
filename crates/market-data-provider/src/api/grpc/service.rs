use crate::proto::{PriceUpdate, market_data_provider_server::MarketDataProvider};
use eyre::Result;
use tokio::sync::broadcast::Sender;
use tonic::{Request, Response, Status, Streaming};
use tracing::info;

pub struct MarketDataProviderService {
    price_sender_channel: Sender<PriceUpdate>,
}

impl MarketDataProviderService {
    pub fn new(price_sender_channel: Sender<PriceUpdate>) -> Self {
        Self {
            price_sender_channel,
        }
    }
}

#[tonic::async_trait]
impl MarketDataProvider for MarketDataProviderService {
    async fn publish_price(
        &self,
        request: Request<Streaming<PriceUpdate>>,
    ) -> Result<Response<()>, Status> {
        let mut prices = request.into_inner();

        loop {
            match prices.message().await {
                Ok(Some(price)) => {
                    info!(
                        instrument = price.instrument,
                        price = price.value,
                        "Price update"
                    );

                    let _ = self.price_sender_channel.send(price);
                }
                Ok(None) => return Ok(Response::new(())),
                Err(error) => return Err(error),
            }
        }
    }
}
