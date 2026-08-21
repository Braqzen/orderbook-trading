use crate::{
    api::{MarketPrice, Trade},
    trade::{Inventory, Order, OrderType, Quantity},
};
use eyre::Result;
use std::collections::HashMap;
use tokio::{
    select,
    sync::mpsc::{Receiver, Sender},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct Engine {
    id: Uuid,
    inventory: Inventory,
    open_orders: HashMap<Uuid, Order>,
    price_receiver_channel: Receiver<MarketPrice>,
    order_sender_channel: Sender<Order>,
    trade_receiver_channel: Receiver<Trade>,
}

impl Engine {
    pub fn new(
        inventory: Inventory,
        price_receiver_channel: Receiver<MarketPrice>,
        order_sender_channel: Sender<Order>,
        trade_receiver_channel: Receiver<Trade>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            inventory,
            open_orders: HashMap::new(),
            price_receiver_channel,
            order_sender_channel,
            trade_receiver_channel,
        }
    }

    pub async fn run(mut self, token: CancellationToken) -> Result<()> {
        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                trade = self.trade_receiver_channel.recv() => {
                    let Some(trade) = trade else {
                        error!("Orderbook trade channel closed");
                        break;
                    };

                    let Some(order) = self.open_orders.get(&trade.order_id).cloned() else {
                        warn!(order = %trade.order_id, "Received fill for unknown order");
                        continue;
                    };

                    let fill_price = trade.price as f64 / 100.0;
                    let fill_size = match Quantity::try_from(trade.size as f64) {
                        Ok(fill_size) => fill_size,
                        Err(error) => {
                            warn!(%error, order = %trade.order_id, "Invalid fill size");
                            continue;
                        }
                    };

                    let result = match order.side {
                        OrderType::Buy => self.inventory.apply_buy(
                            order.instrument.base(),
                            order.instrument.quote(),
                            fill_size,
                            fill_price,
                            order.price,
                        ),
                        OrderType::Sell => self.inventory.apply_sell(
                            order.instrument.base(),
                            order.instrument.quote(),
                            fill_size,
                            fill_price,
                        ),
                    };

                    match result {
                        Ok(()) => {
                            if trade.remaining == 0 {
                                self.open_orders.remove(&trade.order_id);
                            }
                        }
                        Err(error) => {
                            warn!(
                                %error,
                                order = %trade.order_id,
                                "Failed to apply fill to inventory"
                            );
                        }
                    }
                }

                price = self.price_receiver_channel.recv() => {
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

                    if self.order_sender_channel.send(order.clone()).await.is_err() {
                        error!("Order channel closed");
                        break;
                    }

                    self.open_orders.insert(order.order_id, order);

                    let (asset, amount) = match side {
                        OrderType::Buy => (instrument.quote(), value * size as f64),
                        OrderType::Sell => (instrument.base(), size as f64),
                    };

                    let amount = match Quantity::try_from(amount) {
                        Ok(amount) => amount,
                        Err(error) => {
                            warn!(%error, %instrument, %side, size, value, "Invalid reserve amount");
                            continue;
                        }
                    };

                    if let Err(error) = self.inventory.reserve(asset, amount) {
                        warn!(%error, %instrument, %side, size, value, "Failed to reserve inventory");
                    }
                }
            }
        }

        Ok(())
    }
}
