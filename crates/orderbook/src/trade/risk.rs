use crate::trade::{Instrument, LimitOrder, Price};

pub struct RiskAnalyser {
    instrument: Instrument,
}

impl RiskAnalyser {
    pub fn new(instrument: Instrument) -> Self {
        Self { instrument }
    }

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

#[derive(Debug)]
pub enum RejectionReason {
    InvalidInstrument,
}
