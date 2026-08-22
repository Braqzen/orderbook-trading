use crate::{
    proto::{PriceUpdate, generator_feed_server::GeneratorFeed},
    state::State,
};
use eyre::Result;
use std::sync::Arc;
use tonic::{Request, Response, Status, Streaming};
use tracing::info;

pub struct MyGeneratorFeed {
    pub state: Arc<State>,
}

impl MyGeneratorFeed {
    pub fn new(state: Arc<State>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl GeneratorFeed for MyGeneratorFeed {
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
