use crate::trade::{
    level::PriceLevel,
    order::{Order, OrderType},
    price::Price,
};
use std::collections::{HashMap, hash_map::Entry};

pub struct OrderBook {
    buy: HashMap<Price, PriceLevel>,
    sell: HashMap<Price, PriceLevel>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            buy: HashMap::default(),
            sell: HashMap::default(),
        }
    }

    pub fn add_order(&mut self, price: Price, order: Order) -> Result<(), String> {
        let book = match order.side {
            OrderType::Buy => &mut self.buy,
            OrderType::Sell => &mut self.sell,
        };

        match book.entry(price) {
            Entry::Vacant(entry) => {
                entry.insert(PriceLevel::new()).add(order);
                Ok(())
            }
            Entry::Occupied(mut entry) => {
                entry.get_mut().add(order);
                Ok(())
            }
        }
    }
}
