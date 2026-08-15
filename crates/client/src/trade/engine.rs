use super::{Order, OrderType};
use eyre::Result;
use tokio::{
    select,
    sync::mpsc::{Receiver, Sender},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub struct Engine;

impl Engine {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(
        &self,
        mut receiver: Receiver<f64>,
        sender: Sender<Order>,
        token: CancellationToken,
    ) -> Result<()> {
        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                price = receiver.recv() => {
                    let Some(price) = price else {
                        error!("Market feed channel closed");
                        break;
                    };

                    let side = if rand::random_bool(0.5) {
                        OrderType::Buy
                    } else {
                        OrderType::Sell
                    };
                    let size = rand::random_range(1..5);

                    let order = Order::new(price, size, side);

                    info!(price, size, %side, "Created order");

                    if sender.send(order).await.is_err() {
                        error!("Order channel closed");
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}
