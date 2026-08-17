use crate::trade::{ExecutionResult, Order, OrderBook, Price};
use eyre::Result;
use tokio::{select, sync::mpsc::Receiver};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

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

                    let trades = self.book.trade(price, order);

                    let (status, fill_count, remaining_size) = match trades {
                        ExecutionResult::Filled { fills } => {
                            ("filled", fills.len(), 0)
                        },
                        ExecutionResult::PartiallyFilled { fills, remainder } => {
                            ("partial", fills.len(), remainder.size)
                        },
                        ExecutionResult::Unfilled { .. } => {
                            ("unfilled", 0, order.size)
                        },
                    };
                    let filled_size = order.size - remaining_size;

                    info!(
                        limit_price = %price,
                        requested_size = order.size,
                        filled_size,
                        remaining_size,
                        fill_count,
                        side = %order.side,
                        status,
                        client=%order.client_id,
                        order=%order.order_id,
                        "Order processed"
                    );
                }
            }
        }

        Ok(())
    }
}
