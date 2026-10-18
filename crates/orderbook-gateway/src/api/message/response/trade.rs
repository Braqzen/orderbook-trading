use crate::trade::{OrderType, Price, Quantity};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Wire type storing information about a trade
#[derive(Deserialize, Serialize)]
pub struct Trade {
    pub exec_seq: u64,
    pub order_id: Uuid,
    pub side: OrderType,
    pub price: Price,
    pub size: Quantity,
    pub remaining: Quantity,
}
