use crate::{
    api::MarketPrice,
    config::TradeLimit,
    trade::{Asset, Inventory, ORDER_SIZE_ATOM_STEP, OrderType, Quantity, TradeAction},
};
use std::collections::HashMap;

pub struct Trader {
    limits: HashMap<Asset, TradeLimit>,
}

impl Trader {
    pub fn new(limits: HashMap<Asset, TradeLimit>) -> Self {
        Self { limits }
    }

    pub fn evaluate(&self, price: MarketPrice, inventory: &Inventory) -> TradeAction {
        // TODO: strategies will be implemented later, rn we only care the instrument exists
        //       this causes errors because we do not check amounts but nothing should break
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

        let size = rand::random_range(
            limit.minimum_size.atoms() / ORDER_SIZE_ATOM_STEP
                ..=limit.maximum_size.atoms() / ORDER_SIZE_ATOM_STEP,
        ) * ORDER_SIZE_ATOM_STEP;

        TradeAction::Place {
            instrument: price.instrument,
            price: price.value,
            size: Quantity::from(size),
            side,
        }
    }
}
