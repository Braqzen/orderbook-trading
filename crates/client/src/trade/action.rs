use crate::trade::{Instrument, OrderType, Price, Quantity};

pub enum TradeAction {
    Place {
        instrument: Instrument,
        price: Price,
        size: Quantity,
        side: OrderType,
    },
    Skip,
}
