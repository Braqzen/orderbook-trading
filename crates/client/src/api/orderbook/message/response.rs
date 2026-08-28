use crate::trade::{Instrument, OrderType};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Response {
    Trade(Trade),
    OrderAccepted(OrderAccepted),
    OrderRejected(OrderRejection),
    Cancelled(Cancelled),
    CancelRejected(CancelRejection),
}

#[derive(Deserialize, Clone)]
pub struct Trade {
    pub order_id: Uuid,
    pub side: OrderType,
    pub price: u64,
    pub size: u64,
    pub remaining: u64,
}

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

#[derive(Deserialize, Clone)]
pub struct Cancelled {
    pub order_id: Uuid,
}

#[derive(Deserialize)]
pub struct CancelRejection {
    pub order_id: Uuid,
    pub reason: CancelRejectionReason,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
    InvalidInstrument,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelRejectionReason {
    OrderNotFound,
}
