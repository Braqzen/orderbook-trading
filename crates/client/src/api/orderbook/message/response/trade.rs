use crate::trade::OrderType;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Clone)]
pub struct Trade {
    pub order_id: Uuid,
    pub side: OrderType,
    pub price: u64,
    pub size: u64,
    pub remaining: u64,
}
