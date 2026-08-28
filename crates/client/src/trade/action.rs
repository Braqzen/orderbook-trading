use crate::trade::{Instrument, OrderType, Price, Quantity};
use uuid::Uuid;

pub enum TradeAction {
    Place {
        instrument: Instrument,
        price: Price,
        size: Quantity,
        side: OrderType,
    },
    Cancel {
        order_id: Uuid,
    },
    Skip,
}
