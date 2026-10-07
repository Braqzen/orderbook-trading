use crate::trade::{OrderType, Price};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Clone)]
pub struct Trade {
    pub order_id: Uuid,
    pub side: OrderType,
    pub price: Price,
    pub size: u64,
    pub remaining: u64,
}
