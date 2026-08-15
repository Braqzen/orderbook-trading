use crate::trade::{Order, OrderBook, Price};
use eyre::Result;
use tokio::{select, sync::mpsc::Receiver};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub struct Engine {
    book: OrderBook,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            book: OrderBook::new(),
        }
    }

    pub async fn run(
        &mut self,
        mut receiver: Receiver<(Price, Order)>,
        token: CancellationToken,
    ) -> Result<()> {
        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                order = receiver.recv() => {
                    let Some((price, order)) = order else {
                        error!("Engine to orderbook api channel closed");
                        break;
                    };

                    match self.book.add_order(price, order) {
                        Ok(()) => {
                            info!(%price, size = order.size, side = %order.side, "New order")
                        },
                        Err(error) => {
                            warn!(%error, "Failed to add order")
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
