use eyre::Result;
use futures_util::StreamExt;
use tokio::{select, sync::mpsc::Sender};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};

pub struct MarketFeed {
    url: String,
}

impl MarketFeed {
    pub fn new(url: String) -> Self {
        Self { url }
    }

    pub async fn run(&self, sender: Sender<f64>, token: CancellationToken) -> Result<()> {
        let (mut stream, _response) = connect_async(self.url.clone()).await?;

        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                message = stream.next() => {
                    match message {
                        Some(Ok(Message::Text(text))) => {
                            let Ok(price) = text.parse::<f64>() else {
                                warn!(%text, "Failed to parse request as f64");
                                continue
                            };

                            if sender.send(price).await.is_err() {
                                error!("Failed to send price through channel");
                                break;
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            error!("Market service explicitly closed connection");
                            break;
                        }
                        Some(Err(error)) => {
                            error!(%error, "Unknown error");
                            break;
                        }
                        None => {
                            error!("Disconnected from market service");
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
            error!(%error, "Failed to close market feed connection");
        }

        Ok(())
    }
}
