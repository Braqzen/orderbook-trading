//! Assets have different units therefore each asset must have its own quantity defined for trading
//!
//! E.g. If BTC is $70k and TSLA is $100 then the size of 1 is a vastly different value for trading

use crate::trade::Quantity;

#[derive(Debug, Clone, Copy)]
pub struct TradeLimit {
    pub minimum_size: Quantity,
    pub maximum_size: Quantity,
}
