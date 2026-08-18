use crate::trade::{Order, Price};
use serde::Serialize;

pub struct RiskAnalyser {
    instrument: String,
}

impl RiskAnalyser {
    pub fn new(instrument: String) -> Self {
        Self { instrument }
    }

    pub fn evaluate(&self, instrument: &String, _order: &Order, _price: &Price) -> RiskResult {
        if instrument != &self.instrument {
            return Err(RejectionReason::InvalidInstrument);
        }

        Ok(())
    }
}

pub type RiskResult = Result<(), RejectionReason>;

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
    InvalidInstrument,
}
