use crate::trade::{LimitOrder, Quantity};
use std::collections::VecDeque;

pub struct PriceLevel {
    orders: VecDeque<LimitOrder>,
}

impl PriceLevel {
    pub fn new() -> Self {
        Self {
            orders: VecDeque::new(),
        }
    }

    pub fn add(&mut self, order: LimitOrder) {
        self.orders.push_back(order);
    }

    pub fn first_order(&mut self) -> Option<&mut LimitOrder> {
        self.orders.front_mut()
    }

    pub fn remove_first_order(&mut self) -> Option<LimitOrder> {
        self.orders.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    pub fn orders(&self) -> impl Iterator<Item = &LimitOrder> {
        self.orders.iter()
    }

    pub fn quantity(&self) -> Result<Quantity, String> {
        let mut quantity = Quantity::ZERO;

        for order in &self.orders {
            quantity = quantity.checked_add(order.size)?;
        }

        Ok(quantity)
    }
}
