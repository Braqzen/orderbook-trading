//! Risk Analyzer evaluates and accepts/rejects the order of a validated client
//!
//! An orderbook may have various checks before it can accept an order for trading

use crate::trade::{LimitOrder, Price};
use serde::Serialize;

pub struct RiskAnalyser;

impl RiskAnalyser {
    pub fn new() -> Self {
        Self
    }

    pub fn evaluate(&self, _order: &LimitOrder, _price: &Price) -> RiskResult {
        Ok(())
    }
}

pub type RiskResult = Result<(), RejectionReason>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
    InvalidInstrument,
    InvalidOrderSize,
}
