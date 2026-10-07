use crate::trade::{Fill, OrderType, Price, Quantity};
use serde::Serialize;
use uuid::Uuid;

/// Wire type storing information about a trade
#[derive(Serialize)]
pub struct Trade {
    pub order_id: Uuid,
    pub side: OrderType,
    pub price: Price,
    pub size: Quantity,
    pub remaining: Quantity,
}

impl Trade {
    pub fn new(price: Price, fill: &Fill) -> Self {
        Self {
            order_id: fill.order_id,
            side: fill.side,
            price,
            size: fill.fill_size,
            remaining: fill.remaining,
        }
    }
}
