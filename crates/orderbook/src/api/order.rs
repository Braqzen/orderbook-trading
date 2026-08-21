use crate::trade::{Instrument, OrderType};
use serde::Deserialize;
use std::num::NonZeroU64;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct RawOrder {
    pub instrument: Instrument,
    pub price: f64,
    pub size: NonZeroU64,
    pub side: OrderType,
    pub client_id: Uuid,
    pub order_id: Uuid,
}
