//! Wire types representing the actions a client may request from the service

use crate::trade::{Instrument, OrderType};
use serde::Deserialize;
use std::num::NonZeroU64;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(untagged)]
pub enum ClientMessage {
    Login(LoginRequest),
    Trading(RawMessage),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub client_id: Uuid,
}

// TODO: need better name
#[derive(Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum RawMessage {
    Place {
        instrument: Instrument,
        price: NonZeroU64,
        size: NonZeroU64,
        side: OrderType,
        order_id: Uuid,
    },
    Cancel {
        order_id: Uuid,
        price: NonZeroU64,
        side: OrderType,
    },
}
