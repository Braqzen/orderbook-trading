//! Describes the desired actions the [`Trader`] may wish to take

use crate::trade::{Instrument, OrderType, Price, Quantity};
use uuid::Uuid;

pub enum TradeAction {
    /// Place a new order on an orderbook
    Place {
        instrument: Instrument,
        price: Price,
        size: Quantity,
        side: OrderType,
    },
    /// Cancel an existing order
    Cancel { order_id: Uuid },
    /// No-op, at this point in time the trader decided it should not take an action
    Skip,
}
