use crate::trade::Instrument;

pub struct MarketPrice {
    pub instrument: Instrument,
    pub value: f64,
}

impl MarketPrice {
    pub fn new(instrument: Instrument, value: f64) -> Self {
        Self { instrument, value }
    }
}
