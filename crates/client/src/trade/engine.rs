use crate::{
    api::MarketPrice,
    trade::{Inventory, Order, OrderType},
};
use eyre::Result;
use tokio::{
    select,
    sync::mpsc::{Receiver, Sender},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use uuid::Uuid;

pub struct Engine {
    id: Uuid,
    inventory: Inventory,
}

impl Engine {
    pub fn new(inventory: Inventory) -> Self {
        Self {
            id: Uuid::new_v4(),
            inventory,
        }
    }

    pub async fn run(
        self,
        mut receiver: Receiver<MarketPrice>,
        sender: Sender<Order>,
        token: CancellationToken,
    ) -> Result<()> {
        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                price = receiver.recv() => {
                    let Some(MarketPrice { instrument, value }) = price else {
                        error!("Market feed channel closed");
                        break;
                    };

                    let base = self.inventory.available(instrument.base());
                    let quote = self.inventory.available(instrument.quote());

                    // TODO: strategies will be implemented later, rn we only care the instrument exists
                    let (_base, _quote) = match (base, quote) {
                        (Some(base), Some(quote)) => {
                            (base, quote)
                        },
                        (_, _) => continue
                    };

                    let side = if rand::random_bool(0.5) {
                        OrderType::Buy
                    } else {
                        OrderType::Sell
                    };
                    let size = rand::random_range(1..5);
                    let order_id = Uuid::new_v4();

                    let order = Order::new(instrument.clone(), value, size, side, self.id, order_id);

                    info!(%instrument, value, size, %side, client=%self.id, order=%order_id, "Created order");

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
