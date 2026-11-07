//! Routes an order action to the orderbook connection for an instrument.

use crate::trade::Order;

#[derive(Clone, Copy)]
pub enum RequestKind {
    Place,
    Cancel,
}

#[derive(Clone)]
pub struct RequestMetadata {
    pub order: Order,
    pub kind: RequestKind,
}

impl RequestMetadata {
    pub fn place(order: &Order) -> Self {
        Self {
            order: order.clone(),
            kind: RequestKind::Place,
        }
    }

    pub fn cancel(order: &Order) -> Self {
        Self {
            order: order.clone(),
            kind: RequestKind::Cancel,
        }
    }
}
