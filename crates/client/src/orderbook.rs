use eyre::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::{select, sync::mpsc::Receiver};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};

pub struct OrderBook {
    url: String,
}

impl OrderBook {
    pub fn new(url: String) -> Self {
        Self { url }
    }

    pub async fn run(&self, mut receiver: Receiver<f64>, token: CancellationToken) -> Result<()> {
        let (mut stream, _response) = connect_async(self.url.clone()).await?;

        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                price = receiver.recv() => {
                    let Some(price) = price else {
                        error!("Market feed channel closed");
                        break;
                    };

                    match stream.send(Message::Text(price.to_string().into())).await {
                        Ok(()) => {},
                        Err(error) => {
                            error!(%error, "Failed to send order to orderbook");
                            break;
                        }
                    }
                }

                message = stream.next() => {
                    match message {
                        Some(Ok(Message::Close(_))) => {
                            error!("Orderbook service explicitly closed connection");
                            break;
                        }
                        Some(Err(error)) => {
                            error!(%error, "Unknown error");
                            break;
                        }
                        None => {
                            error!("Disconnected from orderbook service");
                            break;
                        }
                        _ => {
                            warn!("Skipping unexpected message");
                        }
                    }
                }
            }
        }

        if let Err(error) = stream.close(None).await {
            error!(%error, "Failed to close orderbook connection");
        }

        Ok(())
    }
}
