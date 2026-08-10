use crate::{
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

    pub fn add_level(
        &mut self,
        price: Price,
        level: PriceLevel,
        side: OrderType,
    ) -> Result<(), String> {
        let book = match side {
            OrderType::Buy => &mut self.buy,
            OrderType::Sell => &mut self.sell,
        };

        match book.entry(price) {
            Entry::Vacant(entry) => {
                entry.insert(level);
                Ok(())
            }
            Entry::Occupied(_) => Err("price level already exists".to_owned()),
        }
    }

    pub fn add_order(&mut self, price: Price, order: Order) -> Result<(), String> {
        let book = match order.side {
            OrderType::Buy => &mut self.buy,
            OrderType::Sell => &mut self.sell,
        };

        match book.entry(price) {
            Entry::Vacant(_) => Err("price level does not exist".to_owned()),
            Entry::Occupied(mut entry) => {
                entry.get_mut().add(order);
                Ok(())
            }
        }
    }
}
