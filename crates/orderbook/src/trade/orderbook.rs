use crate::trade::{
    fill::{ExecutionResult, Fill},
    level::PriceLevel,
    order::{Order, OrderType},
    price::Price,
};
use std::collections::BTreeMap;

pub struct OrderBook {
    buy: BTreeMap<Price, PriceLevel>,
    sell: BTreeMap<Price, PriceLevel>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            buy: BTreeMap::default(),
            sell: BTreeMap::default(),
        }
    }

    pub fn trade(&mut self, price: Price, mut new_order: Order) -> ExecutionResult {
        let mut fills = vec![];

        let (opposite_side_book, same_side_book) = match new_order.side {
            OrderType::Buy => (&mut self.sell, &mut self.buy),
            OrderType::Sell => (&mut self.buy, &mut self.sell),
        };

        while 0 < new_order.size {
            let best_price_level = match new_order.side {
                OrderType::Buy => opposite_side_book.first_entry(),
                OrderType::Sell => opposite_side_book.last_entry(),
            };
            let Some(mut best_price_level) = best_price_level else {
                break;
            };

            let fill_price = *best_price_level.key();

            let price_is_matchable = match new_order.side {
                OrderType::Buy => fill_price <= price,
                OrderType::Sell => price <= fill_price,
            };

            if !price_is_matchable {
                break;
            }

            let price_level = best_price_level.get_mut();

            let Some(existing_order) = price_level.first_order() else {
                best_price_level.remove_entry();
                continue;
            };

            let fill_size = new_order.size.min(existing_order.size);
            existing_order.size -= fill_size;
            new_order.size -= fill_size;

            fills.push(Fill::new(fill_price, fill_size));

            if existing_order.filled() {
                price_level.remove_first_order();
            }

            if price_level.is_empty() {
                best_price_level.remove_entry();
            }
        }

        if !new_order.filled() {
            same_side_book
                .entry(price)
                .or_insert_with(PriceLevel::new)
                .add(new_order.clone());
        }

        if fills.is_empty() {
            ExecutionResult::Unfilled { order: new_order }
        } else if !new_order.filled() {
            ExecutionResult::PartiallyFilled {
                fills,
                remainder: new_order,
            }
        } else {
            ExecutionResult::Filled { fills }
        }
    }
}
