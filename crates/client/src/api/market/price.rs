use crate::trade::{Instrument, Price};

pub struct MarketPrice {
    pub instrument: Instrument,
    pub value: Price,
}

impl MarketPrice {
    pub fn new(instrument: Instrument, value: Price) -> Self {
        Self { instrument, value }
    }
}
