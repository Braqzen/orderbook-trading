use crate::trade::{Instrument, OrderType};
use serde::Deserialize;
use std::num::NonZeroU64;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum RawMessage {
    Place {
        instrument: Instrument,
        price: NonZeroU64,
        size: NonZeroU64,
        side: OrderType,
        client_id: Uuid,
        order_id: Uuid,
    },
    Cancel {
        client_id: Uuid,
        order_id: Uuid,
        price: NonZeroU64,
        side: OrderType,
    },
}
