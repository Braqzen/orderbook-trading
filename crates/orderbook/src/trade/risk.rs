//! Risk Analyzer evaluates and accepts/rejects the order of a validated client
//!
//! An orderbook may have various checks before it can accept an order for trading
//! This project does not currently perform any realistic checks and accepts all orders

use crate::trade::{Instrument, LimitOrder, Price};
use serde::Serialize;

pub struct RiskAnalyser {
    instrument: Instrument,
}

impl RiskAnalyser {
    pub fn new(instrument: Instrument) -> Self {
        Self { instrument }
    }

    // TODO: no checks atm so forcing instrument deep into engine instead of websocket boundary
    pub fn evaluate(
        &self,
        instrument: &Instrument,
        _order: &LimitOrder,
        _price: &Price,
    ) -> RiskResult {
        if instrument != &self.instrument {
            return Err(RejectionReason::InvalidInstrument);
        }

        Ok(())
    }
}

pub type RiskResult = Result<(), RejectionReason>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
    InvalidInstrument,
}
