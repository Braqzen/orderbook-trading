//! The client which receives gRPC instrument/price updates from the generator.
//!
//! Business logic handling receiving of gRPC events and setting values on an external cache (valkey).
//!
//! The aggregator acts as the client of the generator because it collects prices from potentially multiple sources
//! i.e. multiple generators. Here we use 1 generator to send many instruments.

use crate::{proto::price_generator_client::PriceGeneratorClient, valkey::Cache};
use eyre::{Result, eyre};
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub struct GrpcClient {
    /// The generator URL the aggregator connects to for price updates
    generator_url: String,

    /// The cache manages interaction with Valkey i.e. setting price updates
    cache: Cache,
}

impl GrpcClient {
    pub fn new(generator_url: String, cache: Cache) -> Self {
        Self {
            generator_url,
            cache,
        }
    }

    pub async fn run(self, token: CancellationToken) -> Result<()> {
        // Connect the transport. The generator does not send prices on connect.
        let mut client = PriceGeneratorClient::connect(self.generator_url).await?;
        // Open an RPC so the generator can stream prices back. A server cannot push until the client calls it.
        let response = client.subscribe_prices(()).await?;

        let mut stream = response.into_inner();

        loop {
            select! {
                biased;

                // We requested a shutdown
                _ = token.cancelled() => return Ok(()),

                message = stream.message() => match message {
                    Ok(Some(price)) => {
                        info!(
                            instrument = price.instrument,
                            price = price.value,
                            "Price update"
                        );

                        if let Err(error) = self.cache.set(&price.instrument, price.value).await {
                            error!(%error, instrument = %price.instrument, "Failed to store price");
                            return Err(error);
                        }
                    }
                    Ok(None) => return Err(eyre!("price stream closed")),
                    Err(error) => return Err(error.into()),
                },
            }
        }
    }
}
