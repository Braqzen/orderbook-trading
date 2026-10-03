use crate::instrument::Instrument;
use serde::{Deserialize, Serialize};

/// A simple wrapper splitting price information from transport information (gRPC [`PriceUpdate`])
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Price {
    pub instrument: Instrument,
    pub value: f64,
}

impl Price {
    pub fn new(instrument: Instrument, value: f64) -> Self {
        Self { instrument, value }
    }
}
