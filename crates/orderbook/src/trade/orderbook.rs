//! In-memory limit order book with buy and sell sides keyed by price.
//! New orders match against the opposite side; unfilled size rests on the book, and resting orders
//! can be cancelled.

use crate::trade::{LimitOrder, Match, OrderType, Price, PriceLevel, Quantity, TradeResult};
use std::collections::BTreeMap;
use uuid::Uuid;

pub struct OrderBook {
    /// Store a list of buy orders by their price key
    buy: BTreeMap<Price, PriceLevel>,
    /// Store a list of sell orders by their price key
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
        // When a new order comes in track the results of the trades with the existing orders
        let mut matches = vec![];

        // Depending on side of incoming order we want the opposite side to trade
        let (opposite_side_book, same_side_book) = match new_order.side {
            OrderType::Buy => (&mut self.sell, &mut self.buy),
            OrderType::Sell => (&mut self.buy, &mut self.sell),
        };

        // Iterate incoming order till filled or cannot fill further
        while Quantity::ZERO < new_order.size {
            let best_price_level = match new_order.side {
                OrderType::Buy => opposite_side_book.first_entry(),
                OrderType::Sell => opposite_side_book.last_entry(),
            };
            let Some(mut best_price_level) = best_price_level else {
                break;
            };

            let fill_price = *best_price_level.key();

            // Depending on the side and price we walk the prices/levels up/down the book
            // E.g. if buy @ $100 then from lowest price up to $100 we trade, vice-versa for sell
            let price_is_matchable = match new_order.side {
                OrderType::Buy => fill_price <= price,
                OrderType::Sell => price <= fill_price,
            };

            // There are no more prices at an acceptable value for the incoming order
            if !price_is_matchable {
                break;
            }

            let price_level = best_price_level.get_mut();
            let Some(existing_order) = price_level.first_order() else {
                best_price_level.remove_entry();
                continue;
            };

            // We may only fill up to the min amount of both orders
            let fill_size = new_order.size.min(existing_order.size);

            // SAFETY: since bounded above it cannot panic.
            existing_order.size -= fill_size;
            new_order.size -= fill_size;

            matches.push(Match::new(
                fill_price,
                existing_order,
                &new_order,
                fill_size,
            ));

            // If existing order has no more size then it has been filled and can be removed
            if existing_order.filled() {
                price_level.remove_first_order();
            }

            // If there are no more orders on this price point we may remove the entire price level
            if price_level.is_empty() {
                best_price_level.remove_entry();
            }
        }

        let remaining = new_order.size;

        // If at end of matching incoming order is not filled then insert into book
        if !new_order.filled() {
            same_side_book
                .entry(price)
                .or_insert_with(PriceLevel::new)
                .add(new_order);
        }

        TradeResult::new(matches, remaining)
    }

    pub fn cancel(
        &mut self,
        client_id: Uuid,
        order_id: Uuid,
        price: Price,
        side: OrderType,
    ) -> bool {
        let book = match side {
            OrderType::Buy => &mut self.buy,
            OrderType::Sell => &mut self.sell,
        };

        let Some(price_level) = book.get_mut(&price) else {
            return false;
        };

        // TODO: can we remove without this encapsulated iteration? What better data structures?
        let removed = price_level
            .remove_by_order_id(client_id, order_id)
            .is_some();

        if price_level.is_empty() {
            book.remove(&price);
        }

        removed
    }

    // TODO: the levels are legacy for metrics and should be refactored into a better (snapshot) design
    pub fn buy_levels(&self) -> impl DoubleEndedIterator<Item = (&Price, &PriceLevel)> {
        self.buy.iter()
    }

    pub fn sell_levels(&self) -> impl DoubleEndedIterator<Item = (&Price, &PriceLevel)> {
        self.sell.iter()
    }
}
