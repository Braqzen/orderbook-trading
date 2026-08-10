use crate::{
    price::PriceManager,
    proto::{PriceUpdate, generator_feed_client::GeneratorFeedClient},
};
use eyre::Result;
use std::time::Duration;
use tokio::{
    signal::unix::{SignalKind, signal},
    sync::mpsc::{self, Sender},
    time::sleep,
};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, info};

const SLEEP: u64 = 10;

pub struct Worker {
    sleep: u64,
    api: String,
    price_manager: PriceManager,
}

impl Worker {
    pub fn new(api: String, start_price: f64, upper_limit: f64, lower_limit: f64) -> Self {
        Self {
            api,
            sleep: SLEEP,
            price_manager: PriceManager::new(start_price, upper_limit, lower_limit),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Handle running locally and interrupting the process with ctrl+c.
        let mut sigint = signal(SignalKind::interrupt())?;
        // Handle running in a container and terminating the process with docker stop.
        let mut sigterm = signal(SignalKind::terminate())?;

        let shutdown = async {
            tokio::select! {
                _ = sigint.recv() => info!("Received interrupt signal"),
                _ = sigterm.recv() => info!("Received terminate signal"),
            }
        };
        tokio::pin!(shutdown);

        let (sender, receiver) = mpsc::channel(128);
        let mut client = GeneratorFeedClient::connect(self.api.clone()).await?;
        let publish = client.publish_price(ReceiverStream::new(receiver));
        tokio::pin!(publish);

        loop {
            // Ignore error try again
            if let Err(error) = self.send_request(&sender).await {
                error!(%error, "failed to queue price");
            }

            tokio::select! {
                biased;

                _ = &mut shutdown => {
                    drop(sender);
                    return match publish.await {
                        Ok(_) => Ok(()),
                        Err(error) => Err(error.into()),
                    };
                }
                result = &mut publish => {
                    return match result {
                        Ok(_) => Err(std::io::Error::other("price stream closed").into()),
                        Err(error) => Err(error.into()),
                    };
                }
                _ = sleep(Duration::from_millis(self.sleep)) => {}
            }
        }
    }

    async fn send_request(&mut self, sender: &Sender<PriceUpdate>) -> Result<()> {
        let price = self.price_manager.next_price();

        sender.send(PriceUpdate { value: price.value }).await?;

        Ok(())
    }
}
