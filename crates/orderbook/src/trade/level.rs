use crate::trade::{LimitOrder, Quantity};
use std::collections::VecDeque;
use uuid::Uuid;

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

    pub fn remove_by_order_id(&mut self, client_id: Uuid, order_id: Uuid) -> Option<LimitOrder> {
        let position = self
            .orders
            .iter()
            .position(|order| order.client_id == client_id && order.order_id == order_id)?;

        self.orders.remove(position)
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
