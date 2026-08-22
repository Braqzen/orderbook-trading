use crate::trade::{Instrument, OrderType};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Response {
    Trade(Trade),
    Rejection(Rejection),
}

#[derive(Deserialize, Clone)]
pub struct Trade {
    pub order_id: Uuid,
    pub side: OrderType,
    pub price: u64,
    pub size: u64,
    pub remaining: u64,
}

#[derive(Deserialize)]
pub struct Rejection {
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
}
