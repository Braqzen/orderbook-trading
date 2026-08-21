use crate::trade::{OrderType, Price};
use serde::Serialize;
use std::fmt::{self, Display, Formatter};
use uuid::Uuid;

#[derive(Serialize)]
pub struct Trade {
    pub order_id: Uuid,
    pub side: OrderType,
    pub price: Price,
    pub size: u64,
    pub remaining: u64,
}

impl Trade {
    pub fn new(order_id: Uuid, side: OrderType, price: Price, size: u64, remaining: u64) -> Self {
        Self {
            order_id,
            side,
            price,
            size,
            remaining,
        }
    }
}

pub enum TradeStatus {
    Unfilled,
    Partial,
    Filled,
}

impl Display for TradeStatus {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unfilled => formatter.write_str("unfilled"),
            Self::Partial => formatter.write_str("partial"),
            Self::Filled => formatter.write_str("filled"),
        }
    }
}

pub struct TradeResult {
    pub trades: Vec<(Uuid, Trade)>,
    pub remaining: u64,
}

impl TradeResult {
    pub fn new(trades: Vec<(Uuid, Trade)>, remaining: u64) -> Self {
        Self { trades, remaining }
    }

    pub fn status(&self) -> TradeStatus {
        if self.trades.is_empty() {
            TradeStatus::Unfilled
        } else if 0 < self.remaining {
            TradeStatus::Partial
        } else {
            TradeStatus::Filled
        }
    }
}
