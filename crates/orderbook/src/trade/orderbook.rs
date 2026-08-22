use crate::trade::{LimitOrder, OrderType, Price, PriceLevel, Trade, TradeResult};
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

    pub fn trade(&mut self, price: Price, mut new_order: LimitOrder) -> TradeResult {
        let mut trades = vec![];

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

            trades.push((
                existing_order.client_id,
                Trade::new(
                    existing_order.order_id,
                    existing_order.side,
                    fill_price,
                    fill_size,
                    existing_order.size,
                ),
            ));
            trades.push((
                new_order.client_id,
                Trade::new(
                    new_order.order_id,
                    new_order.side,
                    fill_price,
                    fill_size,
                    new_order.size,
                ),
            ));

            if existing_order.filled() {
                price_level.remove_first_order();
            }

            if price_level.is_empty() {
                best_price_level.remove_entry();
            }
        }

        let remaining = new_order.size;

        if !new_order.filled() {
            same_side_book
                .entry(price)
                .or_insert_with(PriceLevel::new)
                .add(new_order);
        }

        TradeResult::new(trades, remaining)
    }
}
