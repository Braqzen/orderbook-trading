use crate::{
    proto::{PriceUpdate, market_data_provider_server::MarketDataProvider},
    state::State,
};
use eyre::Result;
use std::sync::Arc;
use tonic::{Request, Response, Status, Streaming};
use tracing::info;

pub struct MarketDataProviderService {
    pub state: Arc<State>,
}

impl MarketDataProviderService {
    pub fn new(state: Arc<State>) -> Self {
        Self { state }
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
                    self.state
                        .prices
                        .write()
                        .await
                        .insert(price.instrument.clone(), price.value);
                    let _ = self.state.price_sender_channel.send(price);
                }
                Ok(None) => return Ok(Response::new(())),
                Err(error) => return Err(error),
            }
        }
    }
}
