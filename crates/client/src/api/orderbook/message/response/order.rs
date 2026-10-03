use crate::trade::{Instrument, OrderType};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Clone)]
pub struct OrderAccepted {
    pub order_id: Uuid,
}

#[derive(Deserialize)]
pub struct OrderRejection {
    pub order_id: Uuid,
    pub instrument: Instrument,
    pub price: u64,
    pub size: u64,
    pub side: OrderType,
    pub reason: RejectionReason,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
    InvalidInstrument,
    InvalidOrderSize,
}
