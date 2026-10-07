//! Internal type indicating user request to send to the engine
//!
//! Keep protocol/wire type separate in case we want to alter the internal type

use crate::trade::{Instrument, LimitOrder, OrderType, Price};
use uuid::Uuid;

pub enum Request {
    Place {
        instrument: Instrument,
        price: Price,
        order: LimitOrder,
    },
    Cancel {
        client_id: Uuid,
        order_id: Uuid,
        price: Price,
        side: OrderType,
    },
}
