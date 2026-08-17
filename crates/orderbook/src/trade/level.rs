use crate::trade::order::Order;
use std::collections::VecDeque;

pub struct PriceLevel {
    orders: VecDeque<Order>,
}

impl PriceLevel {
    pub fn new() -> Self {
        Self {
            orders: VecDeque::new(),
        }
    }

    pub fn add(&mut self, order: Order) {
        self.orders.push_back(order);
    }

    pub fn first_order(&mut self) -> Option<&mut Order> {
        self.orders.front_mut()
    }

    pub fn remove_first_order(&mut self) -> Option<Order> {
        self.orders.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }
}
