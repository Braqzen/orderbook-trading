use crate::trade::{Instrument, OrderType, Price, Quantity, RejectionReason};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
pub struct OrderAccepted {
    pub exec_seq: u64,
    pub order_id: Uuid,
}

#[derive(Deserialize, Serialize)]
pub struct OrderRejection {
    pub exec_seq: u64,
    pub order_id: Uuid,
    pub instrument: Instrument,
    pub price: Price,
    pub size: Quantity,
    pub side: OrderType,
    pub reason: RejectionReason,
}
