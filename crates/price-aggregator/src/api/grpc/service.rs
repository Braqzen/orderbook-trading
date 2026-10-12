//! Business logic handling receiving of gRPC events and setting values on an external cache (valkey)

use crate::{proto::PriceUpdate, proto::price_aggregator_server::PriceAggregator, valkey::Cache};
use tonic::{Request, Response, Status, Streaming};
use tracing::{error, info};

pub struct PriceAggregatorService {
    cache: Cache,
}

impl PriceAggregatorService {
    pub fn new(cache: Cache) -> Self {
        Self { cache }
    }
}

#[tonic::async_trait]
impl PriceAggregator for PriceAggregatorService {
    async fn publish_price(
        &self,
        request: Request<Streaming<PriceUpdate>>,
    ) -> Result<Response<()>, Status> {
        let mut prices = request.into_inner();

        loop {
            // Parks the task until a new message triggers a wake signal
            match prices.message().await {
                Ok(Some(price)) => {
                    info!(
                        instrument = price.instrument,
                        price = price.value,
                        "Price update"
                    );

                    if let Err(error) = self.cache.set(&price.instrument, price.value).await {
                        error!(%error, instrument = %price.instrument, "Failed to store price");
                        return Err(Status::internal(error.to_string()));
                    }
                }
                Ok(None) => return Ok(Response::new(())),
                Err(error) => return Err(error),
            }
        }
    }
}
