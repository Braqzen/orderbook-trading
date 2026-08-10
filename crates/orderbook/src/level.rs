use crate::order::Order;

pub struct PriceLevel {
    orders: Vec<Order>,
}

impl PriceLevel {
    pub fn new() -> Self {
        Self { orders: vec![] }
    }

    pub fn add(&mut self, order: Order) {
        self.orders.push(order);
    }
}
