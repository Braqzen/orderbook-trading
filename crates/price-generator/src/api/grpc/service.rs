//! Business logic handling streaming of generated prices to connected aggregators.

use crate::proto::{PriceUpdate, price_generator_server::PriceGenerator};
use tokio::sync::{
    broadcast::{Receiver, Sender, error::RecvError},
    mpsc,
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

pub struct PriceGeneratorService {
    prices: Sender<PriceUpdate>,
}

impl PriceGeneratorService {
    pub fn new(prices: Sender<PriceUpdate>) -> Self {
        Self { prices }
    }
}

#[tonic::async_trait]
impl PriceGenerator for PriceGeneratorService {
    type SubscribePricesStream = ReceiverStream<Result<PriceUpdate, Status>>;

    async fn subscribe_prices(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::SubscribePricesStream>, Status> {
        // Each subscription gets its own receiver on the existing broadcast channel.
        let price_receiver_channel = self.prices.subscribe();

        // A local task reads that receiver and sends each price through response_sender.
        // Tonic reads response_receiver and sends those prices to the aggregator.
        let (response_sender, response_receiver) = mpsc::channel(128);

        tokio::spawn(client_stream(price_receiver_channel, response_sender));

        Ok(Response::new(ReceiverStream::new(response_receiver)))
    }
}

async fn client_stream(
    mut price_receiver_channel: Receiver<PriceUpdate>,
    response_sender: mpsc::Sender<Result<PriceUpdate, Status>>,
) {
    loop {
        let price = match price_receiver_channel.recv().await {
            Ok(price) => price,
            Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => break,
        };

        if response_sender.send(Ok(price)).await.is_err() {
            break;
        }
    }
}
