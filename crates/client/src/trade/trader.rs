use crate::{
    api::MarketPrice,
    config::TradeLimit,
    trade::{Asset, Inventory, ORDER_SIZE_ATOM_STEP, Order, OrderType, Quantity, TradeAction},
};
use std::collections::HashMap;
use uuid::Uuid;

// TODO: expand to include strategies, perhaps general or per instrument
pub struct Trader {
    limits: HashMap<Asset, TradeLimit>,
}

impl Trader {
    pub fn new(limits: HashMap<Asset, TradeLimit>) -> Self {
        Self { limits }
    }

    pub fn evaluate(
        &self,
        price: MarketPrice,
        inventory: &Inventory,
        open_orders: &HashMap<Uuid, Order>,
    ) -> TradeAction {
        let roll = rand::random::<f64>();

        if roll < 0.5 {
            return TradeAction::Skip;
        }

        if roll < 0.9 {
            return self.place(price, inventory);
        }

        self.cancel(open_orders)
    }

    fn place(&self, price: MarketPrice, inventory: &Inventory) -> TradeAction {
        if inventory.available(price.instrument.base()).is_none()
            || inventory.available(price.instrument.quote()).is_none()
        {
            return TradeAction::Skip;
        }

        let Some(limit) = self.limits.get(price.instrument.base()) else {
            return TradeAction::Skip;
        };

        let side = if rand::random_bool(0.5) {
            OrderType::Buy
        } else {
            OrderType::Sell
        };

        let size =
            rand::random_range(limit.minimum_size.to_decimals()..=limit.maximum_size.to_decimals())
                * ORDER_SIZE_ATOM_STEP;

        TradeAction::Place {
            instrument: price.instrument,
            price: price.value,
            size: Quantity::from(size),
            side,
        }
    }

    fn cancel(&self, open_orders: &HashMap<Uuid, Order>) -> TradeAction {
        if open_orders.is_empty() {
            return TradeAction::Skip;
        }

        let index = rand::random_range(0..open_orders.len());
        let Some(order_id) = open_orders.keys().nth(index).copied() else {
            return TradeAction::Skip;
        };

        TradeAction::Cancel { order_id }
    }
}
