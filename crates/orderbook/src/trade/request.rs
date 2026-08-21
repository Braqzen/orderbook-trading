use crate::trade::{Instrument, LimitOrder, Price};

pub struct Request {
    pub instrument: Instrument,
    pub price: Price,
    pub order: LimitOrder,
}

impl Request {
    pub fn new(instrument: Instrument, price: Price, order: LimitOrder) -> Self {
        Self {
            instrument,
            price,
            order,
        }
    }
}
